use dioxus::prelude::*;

use crate::model::*;

#[server]
pub async fn create_poll(req: CreatePollRequest) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use nanoid::nanoid;
        use crate::db;

        let mut conn = crate::db::get_pool()
            .get()
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        if req.title.is_empty() || req.title.len() > 200 {
            return Err(ServerFnError::new(
                "Title must be between 1 and 200 characters",
            ));
        }
        if let Some(ref desc) = req.description {
            if desc.len() > 2000 {
                return Err(ServerFnError::new(
                    "Description must not exceed 2000 characters",
                ));
            }
        }
        let non_empty: Vec<&String> =
            req.options.iter().filter(|o| !o.trim().is_empty()).collect();
        if non_empty.len() < 2 {
            return Err(ServerFnError::new(
                "At least 2 non-empty options are required",
            ));
        }
        for opt in &non_empty {
            if opt.len() > 200 {
                return Err(ServerFnError::new(
                    "Each option must not exceed 200 characters",
                ));
            }
        }
        if let Some(deadline) = req.deadline {
            if deadline <= Utc::now() {
                return Err(ServerFnError::new("Deadline must be in the future"));
            }
        }

        let share_id = nanoid!(10);
        let option_strings: Vec<String> = non_empty.iter().map(|s| s.to_string()).collect();
        db::insert_poll(
            &mut conn,
            &share_id,
            &req.title,
            req.description.as_deref(),
            req.deadline,
            req.hide_results,
            &option_strings,
        )
        .await
        .map_err(|_| ServerFnError::new("Failed to create poll"))?;

        Ok(share_id)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = req;
        unreachable!()
    }
}

#[server]
pub async fn get_poll(share_id: String) -> Result<PollView, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use crate::db;

        let mut conn = crate::db::get_pool()
            .get()
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let (poll_id, sid, title, description, deadline, hide_results) =
            db::fetch_poll_by_share(&mut conn, &share_id)
                .await
                .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?
                .ok_or(ServerFnError::new("Poll not found"))?;

        let options_data = db::fetch_options_by_poll(&mut conn, poll_id)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let now = Utc::now();
        let closed = deadline.map_or(false, |d| d <= now);

        let options: Vec<OptionView> = options_data
            .into_iter()
            .map(|(id, _, label)| OptionView { id, label })
            .collect();

        Ok(PollView {
            share_id: sid,
            title,
            description,
            deadline,
            hide_results,
            options,
            closed,
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = share_id;
        unreachable!()
    }
}

#[server]
pub async fn submit_vote(submission: BallotSubmission) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use crate::db;

        let mut conn = crate::db::get_pool()
            .get()
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let (poll_id, _, _, _, deadline, _) =
            db::fetch_poll_by_share(&mut conn, &submission.share_id)
                .await
                .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?
                .ok_or(ServerFnError::new("Poll not found"))?;

        if let Some(dl) = deadline {
            if Utc::now() > dl {
                return Err(ServerFnError::new("Poll is closed"));
            }
        }

        let options = db::fetch_options_by_poll(&mut conn, poll_id)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let valid_ids: std::collections::HashSet<i64> =
            options.iter().map(|(id, _, _)| *id).collect();

        for tier in &submission.tiers {
            for opt_id in tier {
                if !valid_ids.contains(opt_id) {
                    return Err(ServerFnError::new(format!(
                        "Invalid option ID: {opt_id}"
                    )));
                }
            }
        }

        db::insert_vote(&mut conn, poll_id, &submission.tiers)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        Ok(())
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = submission;
        unreachable!()
    }
}

#[server]
pub async fn get_results(share_id: String) -> Result<ResultsView, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use chrono::Utc;
        use crate::db;
        use crate::lottery::compute_results;

        let mut conn = crate::db::get_pool()
            .get()
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let (poll_id, _sid, title, description, deadline, hide_results) =
            db::fetch_poll_by_share(&mut conn, &share_id)
                .await
                .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?
                .ok_or(ServerFnError::new("Poll not found"))?;

        let now = Utc::now();
        let closed = deadline.map_or(false, |d| d <= now);

        if hide_results && !closed {
            let options_data = db::fetch_options_by_poll(&mut conn, poll_id)
                .await
                .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

            let options: Vec<OptionView> = options_data
                .into_iter()
                .map(|(id, _, label)| OptionView { id, label })
                .collect();

            return Ok(ResultsView {
                title,
                description,
                deadline,
                hide_results,
                vote_count: 0,
                closed,
                winner: None,
                standings: vec![],
                options,
            });
        }

        let vote_count = db::count_votes(&mut conn, poll_id)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let options_data = db::fetch_options_by_poll(&mut conn, poll_id)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let votes = db::fetch_votes(&mut conn, poll_id)
            .await
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

        let results = compute_results(&options_data, &votes);

        let options: Vec<OptionView> = options_data
            .into_iter()
            .map(|(id, _, label)| OptionView { id, label })
            .collect();

        Ok(ResultsView {
            title,
            description,
            deadline,
            hide_results,
            vote_count,
            closed,
            winner: results.winner_label,
            standings: results.standings,
            options,
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = share_id;
        unreachable!()
    }
}

#[server]
pub async fn get_head_to_heads(
    share_id: String,
    option_id: i64,
) -> Result<Vec<HeadToHead>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use crate::db;
        use crate::lottery::compute_results;
        use chrono::Utc;

        let mut conn = crate::db::get_pool()
            .get()
            .await
            .map_err(|_| ServerFnError::new("Internal error"))?;

        let (poll_id, _, _, _, deadline, hide_results) =
            db::fetch_poll_by_share(&mut conn, &share_id)
                .await
                .map_err(|_| ServerFnError::new("Internal error"))?
                .ok_or(ServerFnError::new("Poll not found"))?;

        let now = Utc::now();
        let closed = deadline.map_or(false, |d| d <= now);
        if hide_results && !closed {
            return Err(ServerFnError::new("Results are hidden until the deadline"));
        }

        let options_data = db::fetch_options_by_poll(&mut conn, poll_id)
            .await
            .map_err(|_| ServerFnError::new("Internal error"))?;

        let votes = db::fetch_votes(&mut conn, poll_id)
            .await
            .map_err(|_| ServerFnError::new("Internal error"))?;

        let results = compute_results(&options_data, &votes);

        results
            .head_to_heads
            .into_iter()
            .find(|(id, _)| *id == option_id)
            .map(|(_, h2h)| h2h)
            .ok_or(ServerFnError::new("Option not found"))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (share_id, option_id);
        unreachable!()
    }
}
