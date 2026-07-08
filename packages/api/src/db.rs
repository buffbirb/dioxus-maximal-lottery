//! Server-only SQLite access.
use std::sync::OnceLock;

use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

static POOL: OnceLock<SqlitePool> = OnceLock::new();

/// Connect to the database and run migrations. Must be called once before
/// any other function in this module, and before the server starts serving
/// requests.
pub async fn init_pool() -> Result<(), sqlx::Error> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://polls.db?mode=rwc".to_string());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    POOL.set(pool)
        .map_err(|_| ())
        .expect("init_pool must only be called once");

    Ok(())
}

fn pool() -> &'static SqlitePool {
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
    pub idx: i64,
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

    let poll_id: i64 = sqlx::query_scalar(
        "INSERT INTO polls (share_id, title, description, deadline, hide_results, created_at)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&share_id)
    .bind(title)
    .bind(description)
    .bind(deadline)
    .bind(hide_results)
    .bind(Utc::now())
    .fetch_one(&mut *tx)
    .await?;

    for (idx, label) in options.iter().enumerate() {
        sqlx::query("INSERT INTO options (poll_id, idx, label) VALUES (?, ?, ?)")
            .bind(poll_id)
            .bind(idx as i64)
            .bind(label)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(share_id)
}

pub async fn fetch_poll_by_share(
    share_id: &str,
) -> Result<Option<(PollRow, Vec<OptionRow>)>, sqlx::Error> {
    let poll = sqlx::query_as::<_, PollRow>(
        "SELECT id, share_id, title, description, deadline, hide_results, created_at
         FROM polls WHERE share_id = ?",
    )
    .bind(share_id)
    .fetch_optional(pool())
    .await?;

    let Some(poll) = poll else {
        return Ok(None);
    };

    let options = sqlx::query_as::<_, OptionRow>(
        "SELECT id, poll_id, idx, label FROM options WHERE poll_id = ? ORDER BY idx",
    )
    .bind(poll.id)
    .fetch_all(pool())
    .await?;

    Ok(Some((poll, options)))
}

pub async fn insert_vote(poll_id: i64, tiers: &[Vec<i64>]) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;

    let vote_id: i64 =
        sqlx::query_scalar("INSERT INTO votes (poll_id, created_at) VALUES (?, ?) RETURNING id")
            .bind(poll_id)
            .bind(Utc::now())
            .fetch_one(&mut *tx)
            .await?;

    for (tier_idx, option_ids) in tiers.iter().enumerate() {
        for &option_id in option_ids {
            sqlx::query("INSERT INTO vote_rankings (vote_id, option_id, tier) VALUES (?, ?, ?)")
                .bind(vote_id)
                .bind(option_id)
                .bind(tier_idx as i64)
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
        tier: i64,
    }

    let rows = sqlx::query_as::<_, RankingRow>(
        "SELECT vr.vote_id AS vote_id, vr.option_id AS option_id, vr.tier AS tier
         FROM vote_rankings vr
         JOIN votes v ON v.id = vr.vote_id
         WHERE v.poll_id = ?
         ORDER BY vr.vote_id",
    )
    .bind(poll_id)
    .fetch_all(pool())
    .await?;

    let mut by_vote: std::collections::BTreeMap<i64, Vec<(i64, i64)>> = Default::default();
    for row in rows {
        by_vote
            .entry(row.vote_id)
            .or_default()
            .push((row.option_id, row.tier));
    }
    Ok(by_vote.into_values().collect())
}

pub async fn count_votes(poll_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM votes WHERE poll_id = ?")
        .bind(poll_id)
        .fetch_one(pool())
        .await
}
