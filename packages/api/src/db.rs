//! Server-only PostgreSQL access via sqlx.
use std::sync::OnceLock;

// Shared with the client, which checks the same shape to tell a dead share
// link from a URL that was never a share link at all.
use crate::domain::{SHARE_ID_ALPHABET, SHARE_ID_LEN};

const MAX_POOL_CONNECTIONS: u32 = 5;

use chrono::{DateTime, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};

static POOL: OnceLock<PgPool> = OnceLock::new();

/// Connect to the database. Must be called once before any other function in
/// this module, and before the server starts serving requests.
pub async fn init_pool() -> Result<(), sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(MAX_POOL_CONNECTIONS)
        .connect(&database_url)
        .await?;

    POOL.set(pool)
        .map_err(|_| ())
        .expect("init_pool must only be called once");

    Ok(())
}

fn pool() -> &'static PgPool {
    POOL.get()
        .expect("database pool not initialized; call init_pool() first")
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PollRow {
    pub id: i64,
    pub share_id: String,
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
    pub vote_cap: Option<i32>,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

/// Error from [`insert_vote`]. Distinguished from a generic database error so
/// callers can map a reached vote cap to a 400 instead of a 500.
#[derive(Debug)]
pub enum InsertVoteError {
    CapReached,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for InsertVoteError {
    fn from(err: sqlx::Error) -> Self {
        InsertVoteError::Db(err)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OptionRow {
    pub id: i64,
    #[allow(dead_code)]
    pub poll_id: i64,
    #[allow(dead_code)]
    pub idx: i32,
    pub label: String,
}

pub struct InsertedPoll {
    pub share_id: String,
    /// The database-assigned id of each option, in the same order as the
    /// `options` slice passed in.
    pub option_ids: Vec<i64>,
}

#[tracing::instrument(skip_all)]
pub async fn insert_poll(
    title: &str,
    description: Option<&str>,
    deadline: DateTime<Utc>,
    hide_results: bool,
    vote_cap: Option<i32>,
    options: &[String],
) -> Result<InsertedPoll, sqlx::Error> {
    let share_id = nanoid::nanoid!(SHARE_ID_LEN, SHARE_ID_ALPHABET);
    let mut tx = pool().begin().await?;

    let poll_id = sqlx::query_scalar!(
        "INSERT INTO polls (share_id, title, description, deadline, hide_results, vote_cap, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        share_id,
        title,
        description,
        deadline,
        hide_results,
        vote_cap,
        Utc::now()
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut option_ids = Vec::with_capacity(options.len());
    for (idx, label) in options.iter().enumerate() {
        let option_id = sqlx::query_scalar!(
            "INSERT INTO options (poll_id, idx, label) VALUES ($1, $2, $3) RETURNING id",
            poll_id,
            idx as i32,
            label
        )
        .fetch_one(&mut *tx)
        .await?;
        option_ids.push(option_id);
    }

    tx.commit().await?;
    Ok(InsertedPoll {
        share_id,
        option_ids,
    })
}

/// Look up a poll by its public share id. Callers that also need its options
/// and/or vote count should fetch those with [`fetch_poll_options`] and
/// [`count_votes`], run concurrently via `tokio::try_join!` since both only
/// depend on `poll.id`.
#[tracing::instrument]
pub async fn fetch_poll_by_share(share_id: &str) -> Result<Option<PollRow>, sqlx::Error> {
    sqlx::query_as!(
        PollRow,
        "SELECT id, share_id, title, description, deadline, hide_results, vote_cap, created_at
         FROM polls WHERE share_id = $1",
        share_id
    )
    .fetch_optional(pool())
    .await
}

#[tracing::instrument]
pub async fn fetch_poll_options(poll_id: i64) -> Result<Vec<OptionRow>, sqlx::Error> {
    sqlx::query_as!(
        OptionRow,
        "SELECT id, poll_id, idx, label FROM options WHERE poll_id = $1 ORDER BY idx",
        poll_id
    )
    .fetch_all(pool())
    .await
}

/// Casts a vote, re-checking the vote cap inside the transaction (holding a
/// row lock on the poll) so two concurrent submissions at the boundary can't
/// both slip in under the cap.
#[tracing::instrument(skip(tiers))]
pub async fn insert_vote(
    poll_id: i64,
    vote_cap: Option<i32>,
    tiers: &[Vec<i64>],
) -> Result<(), InsertVoteError> {
    let mut tx = pool().begin().await?;

    sqlx::query!("SELECT id FROM polls WHERE id = $1 FOR UPDATE", poll_id)
        .fetch_one(&mut *tx)
        .await?;

    if let Some(cap) = vote_cap {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM votes WHERE poll_id = $1"#,
            poll_id
        )
        .fetch_one(&mut *tx)
        .await?;
        if count >= cap as i64 {
            return Err(InsertVoteError::CapReached);
        }
    }

    let vote_id = sqlx::query_scalar!(
        "INSERT INTO votes (poll_id, created_at) VALUES ($1, $2) RETURNING id",
        poll_id,
        Utc::now()
    )
    .fetch_one(&mut *tx)
    .await?;

    for (tier_idx, option_ids) in tiers.iter().enumerate() {
        for &option_id in option_ids {
            sqlx::query!(
                "INSERT INTO vote_rankings (vote_id, option_id, tier) VALUES ($1, $2, $3)",
                vote_id,
                option_id,
                tier_idx as i32,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

/// Every ballot cast for a poll, as lists of `(option_id, tier)` pairs.
/// Ballots that ranked nothing are omitted since an all-abstain ballot can't
/// affect margins; `count_votes` is the source of truth for the vote count.
#[tracing::instrument]
pub async fn fetch_votes(poll_id: i64) -> Result<Vec<Vec<(i64, i64)>>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct RankingRow {
        vote_id: i64,
        option_id: i64,
        tier: i32,
    }

    let rows = sqlx::query_as!(
        RankingRow,
        "SELECT vr.vote_id, vr.option_id, vr.tier
         FROM vote_rankings vr
         JOIN votes v ON v.id = vr.vote_id
         WHERE v.poll_id = $1
         ORDER BY vr.vote_id",
        poll_id
    )
    .fetch_all(pool())
    .await?;

    let mut by_vote: std::collections::BTreeMap<i64, Vec<(i64, i64)>> = Default::default();
    for row in rows {
        by_vote
            .entry(row.vote_id)
            .or_default()
            .push((row.option_id, row.tier as i64));
    }
    Ok(by_vote.into_values().collect())
}

#[tracing::instrument]
pub async fn count_votes(poll_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*) AS "count!" FROM votes WHERE poll_id = $1"#,
        poll_id
    )
    .fetch_one(pool())
    .await
}
