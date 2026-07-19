//! Fullstack server functions for creating polls, casting ballots, and
//! reading results.
use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::db;
#[cfg(feature = "server")]
use crate::lottery;

#[cfg(feature = "server")]
use crate::model::OptionView;
use crate::model::{BallotSubmission, CreatePollRequest, HeadToHead, PollView, ResultsView};

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
pub async fn create_poll(request: CreatePollRequest) -> Result<String, ServerFnError> {
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

    let share_id = db::insert_poll(
        request.title.as_ref(),
        description,
        request.deadline,
        request.hide_results,
        &options,
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(share_id)
}

#[get("/api/polls/{share_id}")]
pub async fn get_poll(share_id: String) -> Result<PollView, ServerFnError> {
    let (poll, options) = db::fetch_poll_by_share(&share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

    let closed = poll
        .deadline
        .map(|d| d <= chrono::Utc::now())
        .unwrap_or(false);

    Ok(PollView {
        share_id: poll.share_id,
        title: poll.title,
        description: poll.description,
        deadline: poll.deadline,
        hide_results: poll.hide_results,
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
pub async fn submit_vote(ballot: BallotSubmission) -> Result<(), ServerFnError> {
    let (poll, options) = db::fetch_poll_by_share(&ballot.share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

    if let Some(deadline) = poll.deadline
        && deadline <= chrono::Utc::now()
    {
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

    db::insert_vote(poll.id, &ballot.tiers)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[get("/api/polls/{share_id}/results")]
pub async fn get_results(share_id: String) -> Result<ResultsView, ServerFnError> {
    let (poll, option_rows) = db::fetch_poll_by_share(&share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

    let closed = poll
        .deadline
        .map(|d| d <= chrono::Utc::now())
        .unwrap_or(false);
    let results_visible = !poll.hide_results || closed;

    let options: Vec<OptionView> = option_rows
        .iter()
        .map(|o| OptionView {
            id: o.id,
            label: o.label.clone(),
        })
        .collect();

    let vote_count = db::count_votes(poll.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let (winner, standings) = if results_visible {
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
        let solved = lottery::solve(&lottery_options, &votes);
        (solved.winner_label, solved.standings)
    } else {
        (None, Vec::new())
    };

    Ok(ResultsView {
        title: poll.title,
        description: poll.description,
        deadline: poll.deadline,
        hide_results: poll.hide_results,
        results_visible,
        vote_count,
        closed,
        winner,
        standings,
        options,
    })
}

#[get("/api/polls/{share_id}/head-to-head/{option_id}")]
pub async fn get_head_to_head(
    share_id: String,
    option_id: i64,
) -> Result<Vec<HeadToHead>, ServerFnError> {
    let (poll, option_rows) = db::fetch_poll_by_share(&share_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| not_found("poll not found"))?;

    let closed = poll
        .deadline
        .map(|d| d <= chrono::Utc::now())
        .unwrap_or(false);
    if poll.hide_results && !closed {
        return Err(bad_request("results are hidden until the poll closes"));
    }

    let options: Vec<lottery::OptionRef> = option_rows
        .iter()
        .map(|o| lottery::OptionRef {
            id: o.id,
            label: o.label.clone(),
        })
        .collect();
    let votes = db::fetch_votes(poll.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let margins = lottery::tally_margins(&options, &votes);
    let solved = lottery::solve(&options, &votes);
    let order: Vec<i64> = solved
        .standings
        .iter()
        .flat_map(|s| s.members.iter().map(|m| m.option_id))
        .collect();

    Ok(lottery::head_to_head_for(
        &options, &margins, option_id, &order,
    ))
}
