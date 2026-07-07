use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePollRequest {
    pub title: String,
    pub description: Option<String>,
    pub options: Vec<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionView {
    pub id: i64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollView {
    pub share_id: String,
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
    pub options: Vec<OptionView>,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotSubmission {
    pub share_id: String,
    pub tiers: Vec<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingMember {
    #[serde(rename = "optionId")]
    pub option_id: i64,
    pub label: String,
    #[serde(rename = "probabilityPct")]
    pub probability_pct: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingSlot {
    #[serde(rename = "rankLabel")]
    pub rank_label: usize,
    pub members: Vec<StandingMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsView {
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    #[serde(rename = "hideResults")]
    pub hide_results: bool,
    #[serde(rename = "voteCount")]
    pub vote_count: i64,
    pub closed: bool,
    pub winner: Option<String>,
    pub standings: Vec<StandingSlot>,
    pub options: Vec<OptionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadToHead {
    #[serde(rename = "optionId")]
    pub option_id: i64,
    pub label: String,
    pub margin: i64,
}
