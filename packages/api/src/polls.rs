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
#[cfg(feature = "server")]
const NOT_FOUND_CODE: u16 = 404;

/// Return a client-error response with a 400 status code.
#[cfg(feature = "server")]
fn bad_request(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError {
        message: message.into(),
        code: BAD_REQUEST_CODE,
        details: None,
    }
}

/// Return a client-error response with a 404 status code.
#[cfg(feature = "server")]
fn not_found(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError {
        message: message.into(),
        code: NOT_FOUND_CODE,
        details: None,
    }
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
    let poll = db::fetch_poll_by_share(&share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

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
    let poll = db::fetch_poll_by_share(&ballot.share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

    let (options, vote_count) =
        tokio::try_join!(db::fetch_poll_options(poll.id), db::count_votes(poll.id))
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    if poll_closed(poll.deadline, poll.vote_cap, vote_count, chrono::Utc::now()) {
        return Err(bad_request("this poll is closed"));
    }

    if let Some(ctx) = dioxus::fullstack::FullstackContext::current()
        && crate::cookies::has_voted(&ctx.parts_mut(), &ballot.share_id)
    {
        return Err(bad_request("you have already voted on this poll"));
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

    if let Some(ctx) = dioxus::fullstack::FullstackContext::current() {
        let secure = crate::cookies::is_https_request(&ctx.parts_mut());
        ctx.add_response_header(
            http::header::SET_COOKIE,
            http::HeaderValue::try_from(crate::cookies::set_voted_header(&ballot.share_id, secure))
                .expect("the fixed cookie string is a valid header value"),
        );
    }

    Ok(())
}

#[get("/api/polls/{share_id}/results")]
#[cfg_attr(feature = "server", tracing::instrument)]
pub async fn get_results(share_id: String) -> Result<ResultsView, ServerFnError> {
    let poll = db::fetch_poll_by_share(&share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

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
