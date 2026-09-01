//! Binary entry point: configuration, wiring, thread supervision.
//!
//! Deliberately thin. Logic that accumulates here belongs in `vj-core` and `runner`.

pub mod runner;
pub use runner::*;

use std::env;
use std::fs;
use std::path::Path;
use vj_render::Backend;
use vj_types::{ShowManifest, Tier, TierBudget};

fn detect_tier() -> Tier {
    if cfg!(target_os = "macos") {
        Tier::Portable
    } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "linux") {
        Tier::Appliance
    } else {
        Tier::Workstation
    }
}

fn budget_for(tier: Tier) -> TierBudget {
    match tier {
        Tier::Appliance => TierBudget::appliance(),
        Tier::Portable => TierBudget::portable(),
        Tier::Workstation => TierBudget::workstation(),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let tier = detect_tier();
    let budget = budget_for(tier);
    let backend = Backend::preferred_for(tier);

    println!(
        "⚡ RPi-VJ starting: tier={:?} backend={} decks={} layers={} hot_slots={}",
        tier,
        backend.name(),
        budget.visible_decks,
        budget.shader_layers,
        budget.hot_decoder_slots
    );

    let show = if let Some(show_pos) = args.iter().position(|a| a == "--show") {
        if let Some(path) = args.get(show_pos + 1) {
            println!("Loading show file: {}", path);
            match fs::read_to_string(path) {
                Ok(content) => match ShowManifest::from_toml(&content) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error parsing show manifest: {}, falling back to default", e);
                        ShowManifest::new("Default Show")
                    }
                },
                Err(e) => {
                    eprintln!("Error reading show file: {}, falling back to default", e);
                    ShowManifest::new("Default Show")
                }
            }
        } else {
            ShowManifest::new("Default Show")
        }
    } else {
        ShowManifest::new("Default Show")
    };

    let mut runner = EngineRunner::new(show, tier);
    println!("EngineRunner initialized for '{}'. Revision={}", runner.show.name, runner.reducer.state.revision);
}
