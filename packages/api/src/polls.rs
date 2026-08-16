//! Fullstack server functions for creating polls, casting ballots, and
//! reading results.
use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::db;
#[cfg(feature = "server")]
use crate::domain::poll_closed;
#[cfg(feature = "server")]
use crate::lottery;

#[cfg(feature = "server")]
use crate::model::OptionView;
use crate::model::{BallotSubmission, CreatePollRequest, PollView, ResultsView};

#[cfg(feature = "server")]
const BAD_REQUEST_CODE: u16 = 400;
// Not gated on the server feature: not_found_kind below reads it on the
// client too, to tell a missing poll from a failed call.
const NOT_FOUND_CODE: u16 = 404;

/// What a 404 from these endpoints means.
///
/// The server decides this - it is the only side that knows the share id
/// format - and the client only picks wording from it. Nothing here gates
/// an API call: every request runs the lookup regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotFoundKind {
    /// A share-id-shaped path with no poll behind it: a link that is dead,
    /// mistyped, or truncated.
    Poll,
    /// A path that was never a share link. `/:share_id` doubles as the URL
    /// catch-all for single-segment paths, so `/about` and `/robots.txt`
    /// arrive at a poll endpoint too.
    Page,
}

impl NotFoundKind {
    const ALL: [Self; 2] = [Self::Poll, Self::Page];

    /// How this kind is spelled on the wire.
    ///
    /// It rides in the error message because dioxus round-trips the whole
    /// `ServerFnError` - message and code included - through the response
    /// body, so no extra payload type is needed. The trade is that these
    /// strings are protocol, not prose: `not_found_kind` matches on them,
    /// and `every_kind_round_trips_through_its_wire_token` pins the pair.
    /// Wording the user reads lives in the web views, not here.
    const fn wire_token(self) -> &'static str {
        match self {
            Self::Poll => "poll not found",
            Self::Page => "page not found",
        }
    }
}

/// Return a client-error response with a 400 status code.
#[cfg(feature = "server")]
fn bad_request(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError {
        message: message.into(),
        code: BAD_REQUEST_CODE,
        details: None,
    }
}

/// Return a 404 carrying which flavour of missing this is.
#[cfg(feature = "server")]
fn not_found(kind: NotFoundKind) -> ServerFnError {
    ServerFnError::ServerError {
        message: kind.wire_token().to_string(),
        code: NOT_FOUND_CODE,
        details: None,
    }
}

/// "No such poll" vs "the call failed" - callers use this to pick a
/// not-found page or a retry, and `None` means retrying is worth offering.
/// A database hiccup must never claim someone's poll was deleted.
///
/// A 404 we did not mint ourselves (a proxy, a stray route) reads as
/// [`NotFoundKind::Poll`]: these endpoints are addressed by share id, so
/// the poll is the thing that was not found.
pub fn not_found_kind(error: &ServerFnError) -> Option<NotFoundKind> {
    match error {
        ServerFnError::ServerError { code, message, .. } if *code == NOT_FOUND_CODE => Some(
            NotFoundKind::ALL
                .into_iter()
                .find(|kind| kind.wire_token() == message)
                .unwrap_or(NotFoundKind::Poll),
        ),
        _ => None,
    }
}

/// Look up a poll by share id, classifying a miss on the way out.
///
/// The shape check lives here rather than in the client so there is one
/// copy of the rule, and so it can change without shipping a new bundle.
/// It runs before the query only to keep scanner traffic - `/wp-login.php`
/// and friends, all of which route through `/:share_id` - off the
/// database; the answer is the same either way, since an id we would never
/// mint cannot be in the table.
#[cfg(feature = "server")]
async fn fetch_poll_or_not_found(share_id: &str) -> Result<db::PollRow, ServerFnError> {
    if !crate::domain::is_share_id_shaped(share_id) {
        return Err(not_found(NotFoundKind::Page));
    }

    db::fetch_poll_by_share(share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found(NotFoundKind::Poll))
}

#[post("/api/polls")]
#[cfg_attr(feature = "server", tracing::instrument(skip(request)))]
pub async fn create_poll(request: CreatePollRequest) -> Result<PollView, ServerFnError> {
    let description = request.description.as_ref().and_then(|d| {
        let trimmed = d.as_ref();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let options: Vec<String> = request
        .options
        .as_ref()
        .iter()
        .map(|label| label.as_ref().to_string())
        .collect();
    let vote_cap = request.vote_cap.map(|cap| *cap.as_ref());

    let inserted = db::insert_poll(
        request.title.as_ref(),
        description,
        request.deadline,
        request.hide_results,
        vote_cap,
        &options,
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let option_views: Vec<OptionView> = inserted
        .option_ids
        .into_iter()
        .zip(options)
        .map(|(id, label)| OptionView { id, label })
        .collect();

    Ok(PollView {
        share_id: inserted.share_id,
        title: request.title.as_ref().to_string(),
        description: description.map(str::to_string),
        deadline: Some(request.deadline),
        hide_results: request.hide_results,
        vote_cap,
        vote_count: 0,
        options: option_views,
        closed: poll_closed(Some(request.deadline), vote_cap, 0, chrono::Utc::now()),
    })
}

#[get("/api/polls/{share_id}")]
#[cfg_attr(feature = "server", tracing::instrument)]
pub async fn get_poll(share_id: String) -> Result<PollView, ServerFnError> {
    let poll = fetch_poll_or_not_found(&share_id).await?;

    let (options, vote_count) =
        tokio::try_join!(db::fetch_poll_options(poll.id), db::count_votes(poll.id))
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    let closed = poll_closed(poll.deadline, poll.vote_cap, vote_count, chrono::Utc::now());

    Ok(PollView {
        share_id: poll.share_id,
        title: poll.title,
        description: poll.description,
        deadline: poll.deadline,
        hide_results: poll.hide_results,
        vote_cap: poll.vote_cap,
        vote_count,
        options: options
            .into_iter()
            .map(|o| OptionView {
                id: o.id,
                label: o.label,
            })
            .collect(),
        closed,
    })
}

#[post("/api/votes")]
#[cfg_attr(
    feature = "server",
    tracing::instrument(skip(ballot), fields(share_id = %ballot.share_id))
)]
pub async fn submit_vote(ballot: BallotSubmission) -> Result<(), ServerFnError> {
    let poll = fetch_poll_or_not_found(&ballot.share_id).await?;

    let (options, vote_count) =
        tokio::try_join!(db::fetch_poll_options(poll.id), db::count_votes(poll.id))
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    if poll_closed(poll.deadline, poll.vote_cap, vote_count, chrono::Utc::now()) {
        return Err(bad_request("this poll is closed"));
    }

    let valid_ids: std::collections::HashSet<i64> = options.iter().map(|o| o.id).collect();
    let mut seen = std::collections::HashSet::new();
    for tier in &ballot.tiers {
        for id in tier {
            if !valid_ids.contains(id) {
                return Err(bad_request("ballot references an unknown option"));
            }
            if !seen.insert(*id) {
                return Err(bad_request("ballot ranks the same option twice"));
            }
        }
    }

    db::insert_vote(poll.id, poll.vote_cap, &ballot.tiers)
        .await
        .map_err(|e| match e {
            db::InsertVoteError::CapReached => bad_request("this poll has reached its vote cap"),
            db::InsertVoteError::Db(e) => ServerFnError::new(e.to_string()),
        })?;

    Ok(())
}

#[get("/api/polls/{share_id}/results")]
#[cfg_attr(feature = "server", tracing::instrument)]
pub async fn get_results(share_id: String) -> Result<ResultsView, ServerFnError> {
    let poll = fetch_poll_or_not_found(&share_id).await?;

    let (option_rows, vote_count) =
        tokio::try_join!(db::fetch_poll_options(poll.id), db::count_votes(poll.id))
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    let closed = poll_closed(poll.deadline, poll.vote_cap, vote_count, chrono::Utc::now());
    let results_visible = !poll.hide_results || closed;

    let options: Vec<OptionView> = option_rows
        .iter()
        .map(|o| OptionView {
            id: o.id,
            label: o.label.clone(),
        })
        .collect();

    let (standings, margins) = if results_visible {
        let lottery_options: Vec<lottery::OptionRef> = option_rows
            .iter()
            .map(|o| lottery::OptionRef {
                id: o.id,
                label: o.label.clone(),
            })
            .collect();
        let votes = db::fetch_votes(poll.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let margins = lottery::tally_margins(&lottery_options, &votes);
        let standings = lottery::standings(&lottery_options, &margins);
        let flat_margins = lottery::flatten_margins(&lottery_options, &margins);

        (standings, flat_margins)
    } else {
        (Vec::new(), Vec::new())
    };

    Ok(ResultsView {
        title: poll.title,
        description: poll.description,
        deadline: poll.deadline,
        hide_results: poll.hide_results,
        vote_cap: poll.vote_cap,
        results_visible,
        vote_count,
        closed,
        standings,
        options,
        margins,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_error(code: u16, message: &str) -> ServerFnError {
        ServerFnError::ServerError {
            message: message.to_string(),
            code,
            details: None,
        }
    }

    /// Every kind survives the wire: what the server writes as the error
    /// message is the kind the client reads back. Covers `not_found` too,
    /// which is server-only, since both go through `wire_token`.
    #[test]
    fn every_kind_round_trips_through_its_wire_token() {
        for kind in NotFoundKind::ALL {
            assert_eq!(
                not_found_kind(&server_error(NOT_FOUND_CODE, kind.wire_token())),
                Some(kind)
            );
        }
    }

    /// A 404 from somewhere else - a proxy, a stray route - still names a
    /// missing poll, because that is what these endpoints address.
    #[test]
    fn unrecognized_not_found_falls_back_to_the_poll() {
        assert_eq!(
            not_found_kind(&server_error(NOT_FOUND_CODE, "Not Found")),
            Some(NotFoundKind::Poll)
        );
    }

    /// The distinction that keeps a database hiccup from announcing that
    /// someone's poll is gone.
    #[test]
    fn other_failures_are_not_a_missing_poll() {
        assert_eq!(not_found_kind(&ServerFnError::new("boom")), None);
        assert_eq!(
            not_found_kind(&server_error(400, "this poll is closed")),
            None
        );
        assert_eq!(
            not_found_kind(&ServerFnError::StreamError("connection reset".into())),
            None
        );
    }
}
