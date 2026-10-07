//! Server-only PostgreSQL access via sqlx.
use std::sync::OnceLock;

const MAX_POOL_CONNECTIONS: u32 = 5;

use chrono::{DateTime, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::share_id::ShareId;

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
    pub share_id: ShareId,
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
    pub share_id: ShareId,
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
    let share_id = ShareId::mint();
    let mut tx = pool().begin().await?;

    let poll_id = sqlx::query_scalar!(
        "INSERT INTO polls (share_id, title, description, deadline, hide_results, vote_cap, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        share_id.as_ref(),
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
        r#"SELECT id, share_id AS "share_id: ShareId", title, description, deadline,
                  hide_results, vote_cap, created_at
         FROM polls WHERE share_id = $1"#,
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

/// Who is casting or checking a vote. Each identity is checked only against
/// its own column, so a signed-in vote never shows up for the browser's token
/// and vice versa.
#[derive(Debug, Clone, Copy)]
pub enum Voter<'a> {
    Token(&'a [u8]),
    User(i64),
}

#[tracing::instrument(skip(voter))]
pub async fn has_voted(poll_id: i64, voter: Voter<'_>) -> Result<bool, sqlx::Error> {
    voted(pool(), poll_id, voter).await
}

async fn voted<'c>(
    executor: impl sqlx::PgExecutor<'c>,
    poll_id: i64,
    voter: Voter<'_>,
) -> Result<bool, sqlx::Error> {
    match voter {
        Voter::Token(hash) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM votes WHERE poll_id = $1 AND token_hash = $2) AS "exists!""#,
                poll_id,
                hash
            )
            .fetch_one(executor)
            .await
        }
        Voter::User(user_id) => {
            sqlx::query_scalar!(
                r#"SELECT EXISTS(SELECT 1 FROM votes WHERE poll_id = $1 AND user_id = $2) AS "exists!""#,
                poll_id,
                user_id
            )
            .fetch_one(executor)
            .await
        }
    }
}

/// Casts a vote. The poll row is locked and its cap re-read inside the
/// transaction, so two concurrent submissions at the boundary can't both slip
/// in under the cap. A voter that already voted is a no-op, which makes a
/// retried submission idempotent.
#[tracing::instrument(skip(voter, tiers))]
pub async fn insert_vote(
    poll_id: i64,
    voter: Option<Voter<'_>>,
    tiers: &[Vec<i64>],
) -> Result<(), InsertVoteError> {
    let mut tx = pool().begin().await?;

    // FOR NO KEY UPDATE serializes voters on this poll without conflicting
    // with the FOR KEY SHARE lock the votes FK takes.
    let vote_cap = sqlx::query_scalar!(
        "SELECT vote_cap FROM polls WHERE id = $1 FOR NO KEY UPDATE",
        poll_id
    )
    .fetch_one(&mut *tx)
    .await?;

    if let Some(voter) = voter
        && voted(&mut *tx, poll_id, voter).await?
    {
        return Ok(());
    }

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

    // The conflict target must match the identity's partial unique index.
    let vote_id = if let Some(Voter::User(user_id)) = voter {
        sqlx::query_scalar!(
            "INSERT INTO votes (poll_id, user_id, created_at) VALUES ($1, $2, $3)
             ON CONFLICT (poll_id, user_id) WHERE user_id IS NOT NULL DO NOTHING
             RETURNING id",
            poll_id,
            user_id,
            Utc::now()
        )
        .fetch_optional(&mut *tx)
        .await?
    } else {
        let token_hash = match voter {
            Some(Voter::Token(hash)) => Some(hash),
            _ => None,
        };
        sqlx::query_scalar!(
            "INSERT INTO votes (poll_id, token_hash, created_at) VALUES ($1, $2, $3)
             ON CONFLICT (poll_id, token_hash) WHERE token_hash IS NOT NULL DO NOTHING
             RETURNING id",
            poll_id,
            token_hash,
            Utc::now()
        )
        .fetch_optional(&mut *tx)
        .await?
    };

    let Some(vote_id) = vote_id else {
        // A concurrent request from the same voter won the race.
        return Ok(());
    };

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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Finds or creates the user behind a provider identity and returns its id.
///
/// - A returning identity refreshes only the profile fields it carries, so a
///   provider that omits them never blanks what an earlier sign-in stored.
/// - Concurrent first sign-ins race on the identity's unique key; the loser
///   rolls back its orphan user and retries, finding the winner's row.
#[tracing::instrument(skip(display_name, avatar_url, fallback_name))]
pub async fn upsert_identity(
    provider: &str,
    provider_user_id: &str,
    display_name: Option<&str>,
    avatar_url: Option<&str>,
    fallback_name: &str,
) -> Result<i64, sqlx::Error> {
    for _ in 0..2 {
        let mut tx = pool().begin().await?;

        let existing = sqlx::query_scalar!(
            "SELECT user_id FROM user_identities WHERE provider = $1 AND provider_user_id = $2",
            provider,
            provider_user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(user_id) = existing {
            sqlx::query!(
                "UPDATE users
                 SET display_name = COALESCE($2, display_name),
                     avatar_url = COALESCE($3, avatar_url)
                 WHERE id = $1",
                user_id,
                display_name,
                avatar_url
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(user_id);
        }

        let user_id = sqlx::query_scalar!(
            "INSERT INTO users (display_name, avatar_url) VALUES ($1, $2) RETURNING id",
            display_name.unwrap_or(fallback_name),
            avatar_url
        )
        .fetch_one(&mut *tx)
        .await?;

        let linked = sqlx::query_scalar!(
            "INSERT INTO user_identities (user_id, provider, provider_user_id)
             VALUES ($1, $2, $3)
             ON CONFLICT (provider, provider_user_id) DO NOTHING
             RETURNING user_id",
            user_id,
            provider,
            provider_user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(user_id) = linked {
            tx.commit().await?;
            return Ok(user_id);
        }
        tx.rollback().await?;
    }
    // The conflicting row was committed before the retry's lookup, so this
    // means it vanished again in between.
    Err(sqlx::Error::RowNotFound)
}

/// Expired rows are left to [`delete_expired_sessions`], not pruned here.
#[tracing::instrument(skip(token_hash))]
pub async fn insert_session(
    user_id: i64,
    token_hash: &[u8],
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        user_id,
        token_hash,
        expires_at
    )
    .execute(pool())
    .await?;
    Ok(())
}

#[tracing::instrument(skip(token_hash))]
pub async fn delete_session(token_hash: &[u8]) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM sessions WHERE token_hash = $1", token_hash)
        .execute(pool())
        .await?;
    Ok(())
}

#[tracing::instrument(skip(token_hash))]
pub async fn fetch_session_user(token_hash: &[u8]) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        UserRow,
        "SELECT u.id, u.display_name, u.avatar_url
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.expires_at > NOW()",
        token_hash
    )
    .fetch_optional(pool())
    .await
}

/// Returns how many rows were deleted.
#[tracing::instrument]
pub async fn delete_expired_sessions() -> Result<u64, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM sessions WHERE expires_at <= NOW()")
        .execute(pool())
        .await?;
    Ok(result.rows_affected())
}
