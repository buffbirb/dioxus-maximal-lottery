//! Shared DTOs sent between the client and server. Always compiled (no `server`
//! feature gating) so the wasm client can encode/decode them too.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Description, Options, Title};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePollRequest {
    pub title: Title,
    pub description: Option<Description>,
    pub options: Options,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionView {
    pub id: i64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PollView {
    pub share_id: String,
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
    pub options: Vec<OptionView>,
    pub closed: bool,
}

/// A submitted ballot. `tiers` is ordered top (most preferred) to bottom;
/// each inner vector holds the option ids tied within that tier. Options not
/// present anywhere in `tiers` are left unranked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallotSubmission {
    pub share_id: String,
    pub tiers: Vec<Vec<i64>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandingMember {
    pub option_id: i64,
    pub label: String,
    /// Only populated for multi-member (shared/cycle) slots.
    pub probability_pct: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandingSlot {
    pub rank_label: usize,
    pub members: Vec<StandingMember>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultsView {
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
    /// Whether standings/vote_count/winner are populated. False while a poll
    /// has `hide_results` set and its deadline has not yet passed.
    pub results_visible: bool,
    pub vote_count: i64,
    pub closed: bool,
    pub winner: Option<String>,
    pub standings: Vec<StandingSlot>,
    pub options: Vec<OptionView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadToHead {
    pub option_id: i64,
    pub label: String,
    /// Positive = the target option wins this pairing, negative = loses, zero = tie.
    pub margin: i64,
}
