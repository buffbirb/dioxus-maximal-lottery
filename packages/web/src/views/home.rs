use dioxus::prelude::*;

use crate::Route;

#[component]
pub fn Home() -> Element {
    rsx! {
        div { class: "home-page",
            section { class: "hero-section",
                h1 { class: "hero-title", "MaxiLot" }
                p { class: "hero-subtitle",
                    "Create polls. Rank choices. Get fair results."
                }
                Link { to: Route::Create {}, class: "cta-btn", "Create a Poll" }
            }

            section { class: "info-section",
                h2 { "What is this?" }
                p {
                    "MaxiLot lets you create polls where voters rank options into tiers. "
                    "Instead of just picking one winner, voters express their full preferences "
                    "by grouping options they consider equivalent and ranking the groups."
                }

                h2 { "How it works" }
                div { class: "feature-grid",
                    div { class: "feature-card",
                        h3 { "1. Create a Poll" }
                        p { "Add a title, description, and at least 2 options. Set an optional deadline." }
                    }
                    div { class: "feature-card",
                        h3 { "2. Vote by Ranking" }
                        p { "Drag options into tiers to express your preferences. Top tier is best." }
                    }
                    div { class: "feature-card",
                        h3 { "3. Maximal Lottery Results" }
                        p { "The winner is chosen using the maximal lottery method — a fair, strategy-resistant voting system." }
                    }
                }

                h2 { "What is Maximal Lottery?" }
                p {
                    "The maximal lottery is a probabilistic voting method. It picks a probability distribution "
                    "over candidates such that no candidate would expect to lose against a challenger drawn from "
                    "that distribution. When there's a Condorcet winner (someone who beats everyone head-to-head), "
                    "they win with 100% probability. Otherwise, it finds the optimal mixed strategy."
                }

                h2 { "Features" }
                ul { class: "feature-list",
                    li { "Tier-based ranking — group equivalent options together" }
                    li { "Flexible drag-and-drop interface" }
                    li { "Real-time standings with Condorcet cycle detection" }
                    li { "Head-to-head margin breakdowns for every option" }
                    li { "Optional deadlines with countdown timers" }
                    li { "Results can be hidden until deadline" }
                    li { "Dark mode support" }
                }
            }
        }
    }
}
