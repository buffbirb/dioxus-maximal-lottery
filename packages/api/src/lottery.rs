//! Server-only: turns stored ballots into margins, a winner, peeled
//! standings, and head-to-head comparisons using the `maximal-lottery`
//! solver crate.
use std::collections::HashMap;

use maximal_lottery::prelude::*;
use num_traits::ToPrimitive;

use crate::model::{StandingMember, StandingSlot};

pub struct OptionRef {
    pub id: i64,
    pub label: String,
}

pub struct Solved {
    pub winner_label: Option<String>,
    pub standings: Vec<StandingSlot>,
}

fn index_of(options: &[OptionRef]) -> HashMap<i64, usize> {
    options.iter().enumerate().map(|(i, o)| (o.id, i)).collect()
}

/// Build a `PairwiseBallot` per vote: within a ballot, an option in a
/// strictly-higher tier beats every option in a strictly-lower tier (options
/// tied in the same tier abstain against each other), and every ranked
/// option beats every option the ballot left unranked.
fn build_profile(
    options: &[OptionRef],
    votes: &[Vec<(i64, i64)>],
    idx_of: &HashMap<i64, usize>,
) -> PreferenceProfile<PairwiseBallot> {
    let n = options.len();
    let mut ballots = Vec::with_capacity(votes.len());

    for rankings in votes {
        let mut ballot = PairwiseBallot::new(n);
        let ranked_ids: std::collections::HashSet<i64> =
            rankings.iter().map(|&(id, _)| id).collect();

        for i in 0..rankings.len() {
            for j in (i + 1)..rankings.len() {
                let (opt_a, tier_a) = rankings[i];
                let (opt_b, tier_b) = rankings[j];
                if tier_a == tier_b {
                    continue;
                }
                let (winner, loser) = if tier_a < tier_b {
                    (opt_a, opt_b)
                } else {
                    (opt_b, opt_a)
                };
                ballot.set_preference(
                    Candidate(idx_of[&winner]),
                    Candidate(idx_of[&loser]),
                    PairPreference::Left,
                );
            }
        }

        for &(ranked_id, _tier) in rankings {
            for option in options {
                if !ranked_ids.contains(&option.id) {
                    ballot.set_preference(
                        Candidate(idx_of[&ranked_id]),
                        Candidate(idx_of[&option.id]),
                        PairPreference::Left,
                    );
                }
            }
        }

        ballots.push(ballot);
    }

    PreferenceProfile::try_new(ballots).expect("all ballots share the poll's candidate count")
}

pub fn tally_margins(options: &[OptionRef], votes: &[Vec<(i64, i64)>]) -> MarginMatrix {
    let idx_of = index_of(options);
    build_profile(options, votes, &idx_of).tally_margins()
}

pub fn solve(options: &[OptionRef], votes: &[Vec<(i64, i64)>]) -> Solved {
    if votes.is_empty() {
        let members = options
            .iter()
            .map(|o| StandingMember {
                option_id: o.id,
                label: o.label.clone(),
                probability_pct: None,
            })
            .collect();
        return Solved {
            winner_label: None,
            standings: vec![StandingSlot {
                rank_label: 1,
                members,
            }],
        };
    }

    let margins = tally_margins(options, votes);

    let winner_label = margins
        .condorcet_winner()
        .map(|c| c.0)
        .or_else(|| {
            let lottery = CentroidSolver.solve(&margins).ok()?;
            let probs: Vec<&num_rational::BigRational> = (0..options.len())
                .map(|i| {
                    lottery
                        .get(Candidate(i))
                        .expect("every candidate has a probability")
                })
                .collect();
            let max = probs.iter().copied().max()?;
            let mut winners = (0..options.len()).filter(|&i| probs[i] == max);
            let winner = winners.next()?;
            winners.next().is_none().then_some(winner)
        })
        .map(|i| options[i].label.clone());

    let standings = compute_standings(options, &margins);

    Solved {
        winner_label,
        standings,
    }
}

/// Repeatedly peel off either the remaining candidate that beats every other
/// remaining candidate (a singleton slot), or, when there is none, the
/// top-cycle (Smith set) of the remaining sub-election as one shared slot
/// ordered by descending maximal-lottery probability.
fn compute_standings(options: &[OptionRef], margins: &MarginMatrix) -> Vec<StandingSlot> {
    let n = margins.n_candidates();
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut slots = Vec::new();
    let mut placed = 0usize;

    while !remaining.is_empty() {
        let singleton_winner = remaining.iter().copied().find(|&a| {
            remaining
                .iter()
                .all(|&b| a == b || margins.get(Candidate(a), Candidate(b)).unwrap() > 0)
        });

        if let Some(winner) = singleton_winner {
            slots.push(StandingSlot {
                rank_label: placed + 1,
                members: vec![StandingMember {
                    option_id: options[winner].id,
                    label: options[winner].label.clone(),
                    probability_pct: None,
                }],
            });
            placed += 1;
            remaining.retain(|&x| x != winner);
        } else {
            let smith = smith_set(&remaining, margins);
            let sub_margins = restrict_margins(&smith, margins);
            let lottery = CentroidSolver.solve(&sub_margins).ok();

            let mut members: Vec<(usize, Option<num_rational::BigRational>)> = smith
                .iter()
                .enumerate()
                .map(|(i, &orig)| {
                    let prob = lottery.as_ref().and_then(|l| l.get(Candidate(i)).cloned());
                    (orig, prob)
                })
                .collect();
            members.sort_by(|a, b| b.1.cmp(&a.1));

            let slot_size = smith.len();
            let slot_members = members
                .into_iter()
                .map(|(orig, prob)| StandingMember {
                    option_id: options[orig].id,
                    label: options[orig].label.clone(),
                    probability_pct: prob.map(|p| format_pct(&p)),
                })
                .collect();

            slots.push(StandingSlot {
                rank_label: placed + 1,
                members: slot_members,
            });
            placed += slot_size;
            remaining.retain(|x| !smith.contains(x));
        }
    }

    slots
}

/// The Smith set (top cycle) of `remaining`: the smallest non-empty set of
/// candidates each of which beats every candidate outside the set. Computed
/// as the union of strongly-connected components with no incoming edge in
/// the "beats" graph (edge `i -> j` iff `margin(i, j) > 0`).
#[allow(clippy::needless_range_loop)]
fn smith_set(remaining: &[usize], margins: &MarginMatrix) -> Vec<usize> {
    let k = remaining.len();
    if k <= 1 {
        return remaining.to_vec();
    }

    let beats = |a: usize, b: usize| {
        margins
            .get(Candidate(remaining[a]), Candidate(remaining[b]))
            .unwrap()
            > 0
    };

    let mut reach = vec![vec![false; k]; k];
    for (a, row) in reach.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            if a != b && beats(a, b) {
                *cell = true;
            }
        }
    }
    for m in 0..k {
        for a in 0..k {
            for b in 0..k {
                if reach[a][m] && reach[m][b] {
                    reach[a][b] = true;
                }
            }
        }
    }

    let mut scc_id = vec![usize::MAX; k];
    let mut n_scc = 0usize;
    for i in 0..k {
        if scc_id[i] != usize::MAX {
            continue;
        }
        scc_id[i] = n_scc;
        for j in (i + 1)..k {
            if scc_id[j] == usize::MAX && reach[i][j] && reach[j][i] {
                scc_id[j] = n_scc;
            }
        }
        n_scc += 1;
    }

    let mut has_incoming = vec![false; n_scc];
    for i in 0..k {
        for j in 0..k {
            if scc_id[i] != scc_id[j] && beats(j, i) {
                has_incoming[scc_id[i]] = true;
            }
        }
    }

    (0..k)
        .filter(|&i| !has_incoming[scc_id[i]])
        .map(|i| remaining[i])
        .collect()
}

fn restrict_margins(subset: &[usize], margins: &MarginMatrix) -> MarginMatrix {
    let k = subset.len();
    let mut rows = vec![vec![0i64; k]; k];
    for a in 0..k {
        for b in 0..k {
            if a != b {
                rows[a][b] = margins
                    .get(Candidate(subset[a]), Candidate(subset[b]))
                    .unwrap();
            }
        }
    }
    MarginMatrix::from_vec(rows).expect("restricting a valid margin matrix stays valid")
}

fn format_pct(p: &num_rational::BigRational) -> String {
    let pct = p.to_f64().unwrap_or(0.0) * 100.0;
    format!("{}%", pct.round() as i64)
}

/// Flatten a margin matrix into a row-major `Vec<i64>`.
///
/// The row/column index for each option is its position in `options`. The
/// resulting matrix is antisymmetric with a zero diagonal.
pub fn flatten_margins(options: &[OptionRef], margins: &MarginMatrix) -> Vec<i64> {
    let n = options.len();
    let mut flat = vec![0i64; n * n];
    for i in 0..n {
        for j in 0..n {
            flat[i * n + j] = margins.get(Candidate(i), Candidate(j)).unwrap_or(0);
        }
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(labels: &[&str]) -> Vec<OptionRef> {
        labels
            .iter()
            .enumerate()
            .map(|(i, &label)| OptionRef {
                id: i as i64,
                label: label.to_string(),
            })
            .collect()
    }

    #[test]
    fn condorcet_winner_produces_strict_order() {
        let options = opts(&["A", "B", "C"]);
        // Every ballot ranks A > B > C, so A is the Condorcet winner.
        let ballot = vec![(0i64, 0i64), (1, 1), (2, 2)];
        let votes = vec![ballot.clone(), ballot.clone(), ballot];

        let solved = solve(&options, &votes);
        assert_eq!(solved.winner_label.as_deref(), Some("A"));

        let rank_labels: Vec<usize> = solved.standings.iter().map(|s| s.rank_label).collect();
        assert_eq!(rank_labels, vec![1, 2, 3]);
        for slot in &solved.standings {
            assert_eq!(slot.members.len(), 1);
            assert!(slot.members[0].probability_pct.is_none());
        }
        assert_eq!(solved.standings[0].members[0].label, "A");
        assert_eq!(solved.standings[1].members[0].label, "B");
        assert_eq!(solved.standings[2].members[0].label, "C");
    }

    #[test]
    fn three_way_cycle_shares_one_slot_with_equal_probabilities() {
        let options = opts(&["A", "B", "C"]);
        // Rock-paper-scissors cycle: A>B>C, B>C>A, C>A>B (one ballot each).
        let votes = vec![
            vec![(0i64, 0i64), (1, 1), (2, 2)],
            vec![(1, 0), (2, 1), (0, 2)],
            vec![(2, 0), (0, 1), (1, 2)],
        ];

        let solved = solve(&options, &votes);
        assert_eq!(solved.winner_label, None);
        assert_eq!(solved.standings.len(), 1);

        let slot = &solved.standings[0];
        assert_eq!(slot.rank_label, 1);
        assert_eq!(slot.members.len(), 3);
        for member in &slot.members {
            assert_eq!(member.probability_pct.as_deref(), Some("33%"));
        }
    }

    #[test]
    fn standings_numbering_skips_ranks_consumed_by_a_tie() {
        // A beats everyone; B beats everyone but A; C and D tie with each
        // other but both beat E; E loses to everyone.
        let options = opts(&["A", "B", "C", "D", "E"]);
        let margins = MarginMatrix::from_vec(vec![
            vec![0, 5, 5, 5, 5],
            vec![-5, 0, 5, 5, 5],
            vec![-5, -5, 0, 0, 5],
            vec![-5, -5, 0, 0, 5],
            vec![-5, -5, -5, -5, 0],
        ])
        .unwrap();

        let standings = compute_standings(&options, &margins);
        let rank_labels: Vec<usize> = standings.iter().map(|s| s.rank_label).collect();
        assert_eq!(rank_labels, vec![1, 2, 3, 5]);

        assert_eq!(standings[0].members[0].label, "A");
        assert_eq!(standings[1].members[0].label, "B");

        let tied: Vec<&str> = standings[2]
            .members
            .iter()
            .map(|m| m.label.as_str())
            .collect();
        assert_eq!(tied.len(), 2);
        assert!(tied.contains(&"C") && tied.contains(&"D"));
        for member in &standings[2].members {
            assert!(member.probability_pct.is_some());
        }

        assert_eq!(standings[3].members[0].label, "E");
        assert!(standings[3].members[0].probability_pct.is_none());
    }
}
