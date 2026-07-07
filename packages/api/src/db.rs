use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::pooled_connection::deadpool::{Object, Pool};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use diesel_async::RunQueryDsl;

mod schema {
    diesel::table! {
        polls (id) {
            id -> BigInt,
            share_id -> Varchar,
            title -> Varchar,
            description -> Nullable<Text>,
            deadline -> Nullable<Timestamptz>,
            hide_results -> Bool,
            created_at -> Timestamptz,
        }
    }

    diesel::table! {
        options (id) {
            id -> BigInt,
            poll_id -> BigInt,
            idx -> Integer,
            label -> Varchar,
        }
    }

    diesel::table! {
        votes (id) {
            id -> BigInt,
            poll_id -> BigInt,
            created_at -> Timestamptz,
        }
    }

    diesel::table! {
        vote_rankings (vote_id, option_id) {
            vote_id -> BigInt,
            option_id -> BigInt,
            tier -> Integer,
        }
    }
}

use schema::*;

pub type DbPool = Pool<AsyncPgConnection>;
pub type DbConn = Object<AsyncPgConnection>;

static DB_POOL: std::sync::OnceLock<DbPool> = std::sync::OnceLock::new();

pub fn init_pool(database_url: &str) {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    let pool = Pool::builder(config)
        .build()
        .expect("Failed to create DB pool");
    DB_POOL.set(pool).ok();
}

pub fn get_pool() -> &'static DbPool {
    DB_POOL.get().expect("DB pool not initialized")
}

pub async fn run_migrations() {
    let pool = get_pool();
    let mut conn = pool.get().await.expect("Failed to get DB connection");

    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS polls (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            share_id VARCHAR(21) NOT NULL UNIQUE,
            title VARCHAR(200) NOT NULL,
            description TEXT,
            deadline TIMESTAMPTZ,
            hide_results BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to create polls table");

    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS options (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            poll_id BIGINT NOT NULL REFERENCES polls(id) ON DELETE CASCADE,
            idx INTEGER NOT NULL,
            label VARCHAR(200) NOT NULL,
            UNIQUE (poll_id, idx)
        )"
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to create options table");

    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS votes (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            poll_id BIGINT NOT NULL REFERENCES polls(id) ON DELETE CASCADE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to create votes table");

    diesel::sql_query(
        "CREATE TABLE IF NOT EXISTS vote_rankings (
            vote_id BIGINT NOT NULL REFERENCES votes(id) ON DELETE CASCADE,
            option_id BIGINT NOT NULL REFERENCES options(id) ON DELETE CASCADE,
            tier INTEGER NOT NULL,
            PRIMARY KEY (vote_id, option_id)
        )"
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to create vote_rankings table");
}

#[derive(Insertable)]
#[diesel(table_name = polls)]
struct NewPoll<'a> {
    share_id: &'a str,
    title: &'a str,
    description: Option<&'a str>,
    deadline: Option<DateTime<Utc>>,
    hide_results: bool,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = polls)]
struct Poll {
    id: i64,
    share_id: String,
    title: String,
    description: Option<String>,
    deadline: Option<DateTime<Utc>>,
    hide_results: bool,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = options)]
struct NewOption<'a> {
    poll_id: i64,
    idx: i32,
    label: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = options)]
struct OptionRow {
    id: i64,
    #[allow(dead_code)]
    poll_id: i64,
    idx: i32,
    label: String,
}

#[derive(Insertable)]
#[diesel(table_name = votes)]
struct NewVote {
    poll_id: i64,
}

#[derive(Insertable)]
#[diesel(table_name = vote_rankings)]
struct NewVoteRanking {
    vote_id: i64,
    option_id: i64,
    tier: i32,
}

pub async fn insert_poll(
    conn: &mut DbConn,
    share_id: &str,
    title: &str,
    description: Option<&str>,
    deadline: Option<DateTime<Utc>>,
    hide_results: bool,
    options_data: &[String],
) -> QueryResult<i64> {
    let poll = diesel::insert_into(polls::table)
        .values(NewPoll {
            share_id,
            title,
            description,
            deadline,
            hide_results,
        })
        .returning(polls::id)
        .get_result::<i64>(&mut *conn)
        .await?;

    let option_rows: Vec<NewOption> = options_data
        .iter()
        .enumerate()
        .map(|(idx, label)| NewOption {
            poll_id: poll,
            idx: idx as i32,
            label,
        })
        .collect();

    diesel::insert_into(options::table)
        .values(&option_rows)
        .execute(&mut *conn)
        .await?;

    Ok(poll)
}

pub async fn fetch_poll_by_share(
    conn: &mut DbConn,
    share_id: &str,
) -> QueryResult<Option<(i64, String, String, Option<String>, Option<DateTime<Utc>>, bool)>> {
    let result = polls::table
        .filter(polls::share_id.eq(share_id))
        .select(Poll::as_select())
        .first(&mut *conn)
        .await
        .optional()?;

    Ok(result.map(|p| (p.id, p.share_id, p.title, p.description, p.deadline, p.hide_results)))
}

pub async fn fetch_options_by_poll(
    conn: &mut DbConn,
    poll_id: i64,
) -> QueryResult<Vec<(i64, i32, String)>> {
    let rows = options::table
        .filter(options::poll_id.eq(poll_id))
        .order(options::idx.asc())
        .select(OptionRow::as_select())
        .get_results(&mut *conn)
        .await?;

    Ok(rows.into_iter().map(|r| (r.id, r.idx, r.label)).collect())
}

pub async fn insert_vote(
    conn: &mut DbConn,
    poll_id: i64,
    tiers: &[Vec<i64>],
) -> QueryResult<()> {
    let vote_id = diesel::insert_into(votes::table)
        .values(NewVote { poll_id })
        .returning(votes::id)
        .get_result::<i64>(&mut *conn)
        .await?;

    let mut rankings = Vec::new();
    for (tier_idx, tier) in tiers.iter().enumerate() {
        for option_id in tier {
            rankings.push(NewVoteRanking {
                vote_id,
                option_id: *option_id,
                tier: tier_idx as i32,
            });
        }
    }

    if !rankings.is_empty() {
        diesel::insert_into(vote_rankings::table)
            .values(&rankings)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

pub async fn fetch_votes(
    conn: &mut DbConn,
    poll_id: i64,
) -> QueryResult<Vec<Vec<(i64, i32)>>> {
    let vote_ids: Vec<i64> = votes::table
        .filter(votes::poll_id.eq(poll_id))
        .order(votes::id.asc())
        .select(votes::id)
        .get_results(&mut *conn)
        .await?;

    let mut result = Vec::new();
    for vote_id in vote_ids {
        let rankings: Vec<(i64, i32)> = vote_rankings::table
            .filter(vote_rankings::vote_id.eq(vote_id))
            .select((vote_rankings::option_id, vote_rankings::tier))
            .get_results(&mut *conn)
            .await?;
        result.push(rankings);
    }

    Ok(result)
}

pub async fn count_votes(
    conn: &mut DbConn,
    poll_id: i64,
) -> QueryResult<i64> {
    votes::table
        .filter(votes::poll_id.eq(poll_id))
        .count()
        .get_result(&mut *conn)
        .await
}
