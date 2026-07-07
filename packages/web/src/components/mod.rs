mod countdown;
mod modal;
mod option_chip;
mod share_section;
mod theme;
mod tier_ranker;

pub use countdown::Countdown;
pub use modal::Modal;
pub use share_section::ShareSection;
pub use theme::{cycle_theme, theme_icon, use_theme};
pub use tier_ranker::{OptionInfo, TierRanker, TierRankerOutput};

pub fn current_origin() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().origin().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}
