use dioxus::prelude::*;

use crate::Route;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { id: "home",
            section { class: "hero-section",
                h1 { "Rank your options. Let the lottery decide." }
                p { class: "subtitle",
                    "Create a poll, let voters rank as many or as few options as they like into tiers, "
                    "and get a fair winner backed by the maximal-lottery method - Condorcet standings "
                    "when there's a clear order, and shared probabilities when there isn't."
                }
                Link { to: Route::Create {}, class: "cta-button", "Create a poll" }
            }

            section { class: "explainer-section",
                div { class: "explainer-card",
                    h3 { "Rank, don't just pick one" }
                    p {
                        "Voters drag options into as many tiers as they want - ties are fine, and "
                        "unranked options are simply treated as the voter's least preferred."
                    }
                }
                div { class: "explainer-card",
                    h3 { "How the winner is chosen" }
                    p {
                        "We tally head-to-head margins between every pair of options. If one option "
                        "beats every other option head-to-head, it wins outright. When votes cycle "
                        "(A beats B, B beats C, C beats A), the maximal lottery assigns each option in "
                        "the cycle a fair win probability."
                    }
                }
                div { class: "explainer-card",
                    h3 { "Transparent standings" }
                    p {
                        "Results show a full ranked list of tiers, with win probabilities for any "
                        "options sharing a slot, and head-to-head margins for every matchup."
                    }
                }
            }
        }
    }
}
