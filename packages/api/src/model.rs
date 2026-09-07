//! Shared DTOs sent between the client and server. Always compiled (no `server`
//! feature gating) so the wasm client can encode/decode them too.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Description, Options, Title, VoteCap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePollRequest {
    pub title: Title,
    pub description: Option<Description>,
    pub options: Options,
    pub deadline: DateTime<Utc>,
    pub vote_cap: Option<VoteCap>,
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
    pub vote_cap: Option<i32>,
    pub vote_count: i64,
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
    /// The row's position in the standings, counting from 1. A slot shared by
    /// several options still only takes one number.
    pub rank_label: usize,
    pub members: Vec<StandingMember>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultsView {
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<DateTime<Utc>>,
    pub hide_results: bool,
    pub vote_cap: Option<i32>,
    /// Whether `standings` and `margins` are populated. False while a poll has
    /// `hide_results` set and its deadline has not yet passed.
    pub results_visible: bool,
    pub vote_count: i64,
    pub closed: bool,
    pub standings: Vec<StandingSlot>,
    pub options: Vec<OptionView>,
    /// Head-to-head margins for every pairing, flattened row-major over
    /// `options`: the margin of `options[i]` against `options[j]` lives at
    /// `i * options.len() + j`. Positive = the row option wins that pairing,
    /// negative = loses, zero = tie. Antisymmetric with a zero diagonal.
    ///
    /// Either empty (when `results_visible` is false) or exactly
    /// `options.len()` squared long - index defensively rather than assuming
    /// the latter.
    pub margins: Vec<i64>,
}
