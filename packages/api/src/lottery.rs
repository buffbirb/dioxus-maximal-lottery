use maximal_lottery::ballot::{PairPreference, PairwiseBallot};
use maximal_lottery::prelude::*;
use maximal_lottery::solver::CentroidSolver;
use num_rational::BigRational;
use std::collections::{HashMap, HashSet};

use super::model::*;

#[derive(Debug, Clone)]
pub struct ComputedResults {
    pub winner_label: Option<String>,
    pub standings: Vec<StandingSlot>,
    pub head_to_heads: Vec<(i64, Vec<HeadToHead>)>,
}

pub fn compute_results(
    options: &[(i64, i32, String)],
    votes: &[Vec<(i64, i32)>],
) -> ComputedResults {
    let candidate_index: HashMap<i64, usize> = options
        .iter()
        .map(|(id, idx, _)| (*id, *idx as usize))
        .collect();

    let idx_to_option: HashMap<usize, (i64, String)> = options
        .iter()
        .map(|(id, _, label)| ((candidate_index[id]), (*id, label.clone())))
        .collect();

    let n = options.len();
    if n == 0 {
        return ComputedResults {
            winner_label: None,
            standings: vec![],
            head_to_heads: vec![],
        };
    }

    let ballots: Vec<PairwiseBallot> = votes
        .iter()
        .map(|vote| build_ballot(vote, &candidate_index, n))
        .collect();

    let profile = match PreferenceProfile::try_new(ballots) {
        Ok(p) => p,
        Err(_) => {
            return ComputedResults {
                winner_label: None,
                standings: vec![],
                head_to_heads: vec![],
            };
        }
    };
    let margins = profile.tally_margins();
    let lottery = match CentroidSolver.solve(&margins) {
        Ok(l) => l,
        Err(_) => {
            return ComputedResults {
                winner_label: None,
                standings: vec![],
                head_to_heads: vec![],
            };
        }
    };

    let winner_label = pick_winner(&margins, &lottery, &idx_to_option, n);

    let standings = compute_standings(&margins, &lottery, &idx_to_option, n);

    let head_to_heads: Vec<(i64, Vec<HeadToHead>)> = standings
        .iter()
        .flat_map(|slot| &slot.members)
        .filter_map(|m| {
            let target_idx = *candidate_index.get(&m.option_id)?;
            let h2h: Vec<HeadToHead> = options
                .iter()
                .filter(|(id, _, _)| *id != m.option_id)
                .filter_map(|(other_id, _, other_label)| {
                    let other_idx = *candidate_index.get(other_id)?;
                    let margin = margins
                        .get(Candidate(target_idx), Candidate(other_idx))
                        .unwrap_or(0);
                    Some(HeadToHead {
                        option_id: *other_id,
                        label: other_label.clone(),
                        margin,
                    })
                })
                .collect();
            Some((m.option_id, h2h))
        })
        .collect();

    ComputedResults {
        winner_label,
        standings,
        head_to_heads,
    }
}

fn build_ballot(
    vote: &[(i64, i32)],
    candidate_index: &HashMap<i64, usize>,
    n: usize,
) -> PairwiseBallot {
    let mut tier_map: HashMap<i32, Vec<usize>> = HashMap::new();
    for (opt_id, tier) in vote {
        if let Some(&idx) = candidate_index.get(opt_id) {
            tier_map.entry(*tier).or_default().push(idx);
        }
    }

    let mut candidate_tier: HashMap<usize, Option<i32>> = HashMap::new();
    for (tier, cands) in &tier_map {
        for &c in cands {
            candidate_tier.insert(c, Some(*tier));
        }
    }
    for c in 0..n {
        candidate_tier.entry(c).or_insert(None);
    }

    let mut ballot = PairwiseBallot::new(n);

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let c_i = Candidate(i);
            let c_j = Candidate(j);

            let tier_i = candidate_tier[&i];
            let tier_j = candidate_tier[&j];

            match (tier_i, tier_j) {
                (Some(ti), Some(tj)) if ti < tj => {
                    ballot.set_preference(c_i, c_j, PairPreference::Left);
                }
                (Some(ti), Some(tj)) if ti > tj => {
                    ballot.set_preference(c_i, c_j, PairPreference::Right);
                }
                (Some(_), None) => {
                    ballot.set_preference(c_i, c_j, PairPreference::Left);
                }
                (None, Some(_)) => {
                    ballot.set_preference(c_i, c_j, PairPreference::Right);
                }
                _ => {}
            }
        }
    }

    ballot
}

fn pick_winner(
    margins: &MarginMatrix,
    lottery: &Lottery,
    idx_to_option: &HashMap<usize, (i64, String)>,
    n: usize,
) -> Option<String> {
    if let Some(cw) = margins.condorcet_winner() {
        return idx_to_option.get(&cw.0).map(|(_, label)| label.clone());
    }

    let mut best_idx = 0usize;
    let mut best_prob = BigRational::default();
    for i in 0..n {
        if let Some(prob) = lottery.get(Candidate(i)) {
            if prob > &best_prob {
                best_prob = prob.clone();
                best_idx = i;
            }
        }
    }

    idx_to_option.get(&best_idx).map(|(_, label)| label.clone())
}

fn compute_standings(
    margins: &MarginMatrix,
    _full_lottery: &Lottery,
    idx_to_option: &HashMap<usize, (i64, String)>,
    n: usize,
) -> Vec<StandingSlot> {
    let mut standings: Vec<StandingSlot> = Vec::new();
    let mut remaining: HashSet<usize> = (0..n).collect();
    let mut placed_above = 0usize;

    while !remaining.is_empty() {
        let remaining_vec: Vec<usize> = remaining.iter().copied().collect();
        let condorcet_winner = remaining_vec
            .iter()
            .find(|&&a| {
                remaining_vec
                    .iter()
                    .all(|&b| a == b || margins.get(Candidate(a), Candidate(b)).unwrap_or(0) > 0)
            })
            .copied();

        if let Some(cw) = condorcet_winner {
            if let Some((opt_id, label)) = idx_to_option.get(&cw) {
                standings.push(StandingSlot {
                    rank_label: placed_above + 1,
                    members: vec![StandingMember {
                        option_id: *opt_id,
                        label: label.clone(),
                        probability_pct: None,
                    }],
                });
            }
            remaining.remove(&cw);
            placed_above += 1;
        } else {
            let smith_set = compute_smith_set(margins, &remaining_vec);
            let sub_margins = extract_sub_margins(margins, &smith_set);
            let sub_lottery = if smith_set.len() >= 2 {
                CentroidSolver.solve(&sub_margins).ok()
            } else {
                None
            };

            let mut members: Vec<StandingMember> = smith_set
                .iter()
                .filter_map(|&cand| {
                    let prob_str = sub_lottery.as_ref().and_then(|l| {
                        let smith_idx = smith_set.iter().position(|&x| x == cand).unwrap_or(0);
                        l.get(Candidate(smith_idx)).map(|p| {
                            let pct = p.clone() * BigRational::from_integer(100.into());
                            format!("{:.0}%", pct.to_integer())
                        })
                    });

                    let (opt_id, label) = idx_to_option.get(&cand)?;
                    Some(StandingMember {
                        option_id: *opt_id,
                        label: label.clone(),
                        probability_pct: prob_str,
                    })
                })
                .collect();

            members.sort_by(|a, b| {
                let pa = a
                    .probability_pct
                    .as_ref()
                    .and_then(|s| s.trim_end_matches('%').parse::<i64>().ok())
                    .unwrap_or(0);
                let pb = b
                    .probability_pct
                    .as_ref()
                    .and_then(|s| s.trim_end_matches('%').parse::<i64>().ok())
                    .unwrap_or(0);
                pb.cmp(&pa)
            });

            let slot_size = smith_set.len();
            standings.push(StandingSlot {
                rank_label: placed_above + 1,
                members,
            });

            for cand in &smith_set {
                remaining.remove(cand);
            }
            placed_above += slot_size;
        }
    }

    standings
}

fn compute_smith_set(margins: &MarginMatrix, candidates: &[usize]) -> Vec<usize> {
    let n = candidates.len();
    let _index_map: HashMap<usize, usize> = candidates
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i))
        .collect();

    let mut reachable = vec![vec![false; n]; n];
    for i in 0..n {
        reachable[i][i] = true;
    }
    for i in 0..n {
        for j in 0..n {
            if i != j {
                let margin = margins
                    .get(Candidate(candidates[i]), Candidate(candidates[j]))
                    .unwrap_or(0);
                if margin > 0 {
                    reachable[i][j] = true;
                }
            }
        }
    }

    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                reachable[i][j] = reachable[i][j] || (reachable[i][k] && reachable[k][j]);
            }
        }
    }

    let mut smith: Vec<usize> = Vec::new();
    for i in 0..n {
        let dominated = (0..n).any(|j| {
            i != j
                && margins
                    .get(Candidate(candidates[j]), Candidate(candidates[i]))
                    .unwrap_or(0)
                    > 0
                && !reachable[i][j]
        });
        if !dominated {
            smith.push(candidates[i]);
        }
    }

    smith
}

fn extract_sub_margins(margins: &MarginMatrix, subset: &[usize]) -> MarginMatrix {
    let m = subset.len();

    let mut sub = vec![vec![0i64; m]; m];
    for (i, &ci) in subset.iter().enumerate() {
        for (j, &cj) in subset.iter().enumerate() {
            if i != j {
                sub[i][j] = margins.get(Candidate(ci), Candidate(cj)).unwrap_or(0);
            }
        }
    }

    MarginMatrix::from_vec(sub)
        .unwrap_or_else(|_| MarginMatrix::from_vec(vec![vec![0i64; m]; m]).unwrap())
}
