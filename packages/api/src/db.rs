//! Server-only PostgreSQL access via sqlx.
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};

static POOL: OnceLock<PgPool> = OnceLock::new();

/// Connect to the database. Must be called once before any other function in
/// this module, and before the server starts serving requests.
pub async fn init_pool() -> Result<(), sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
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
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
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

pub async fn insert_poll(
    title: &str,
    description: Option<&str>,
    deadline: Option<DateTime<Utc>>,
    hide_results: bool,
    options: &[String],
) -> Result<String, sqlx::Error> {
    let share_id = nanoid::nanoid!(10);
    let mut tx = pool().begin().await?;

    let poll_id = sqlx::query_scalar!(
        "INSERT INTO polls (share_id, title, description, deadline, hide_results, created_at)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        share_id,
        title,
        description,
        deadline,
        hide_results,
        Utc::now()
    )
    .fetch_one(&mut *tx)
    .await?;

    for (idx, label) in options.iter().enumerate() {
        sqlx::query!(
            "INSERT INTO options (poll_id, idx, label) VALUES ($1, $2, $3)",
            poll_id,
            idx as i32,
            label
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(share_id)
}

pub async fn fetch_poll_by_share(
    share_id: &str,
) -> Result<Option<(PollRow, Vec<OptionRow>)>, sqlx::Error> {
    let poll = sqlx::query_as!(
        PollRow,
        "SELECT id, share_id, title, description, deadline, hide_results, created_at
         FROM polls WHERE share_id = $1",
        share_id
    )
    .fetch_optional(pool())
    .await?;

    let Some(poll) = poll else {
        return Ok(None);
    };

    let options = sqlx::query_as!(
        OptionRow,
        "SELECT id, poll_id, idx, label FROM options WHERE poll_id = $1 ORDER BY idx",
        poll.id
    )
    .fetch_all(pool())
    .await?;

    Ok(Some((poll, options)))
}

pub async fn insert_vote(poll_id: i64, tiers: &[Vec<i64>]) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;

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

pub async fn count_votes(poll_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!("SELECT COUNT(*) FROM votes WHERE poll_id = $1", poll_id)
        .fetch_one(pool())
        .await
}
