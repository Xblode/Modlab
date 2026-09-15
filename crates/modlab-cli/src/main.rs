//! `modlab` — CLI client of the engine (`modlab-core`).
//!
//! Every command should eventually support `--json` so Claude Code can
//! drive this as efficiently as a human — see
//! docs/01-architecture-decidee.md §7.1. Only `games` exists so far, as a
//! stub: no `GameAdapter` has been implemented yet, so detection always
//! reports nothing found rather than pretending to know about Skyrim.

use clap::{Parser, Subcommand};
use modlab_core::Core;

#[derive(Parser)]
#[command(name = "modlab", version, about = "ModLab engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List detected games. Stub: no GameAdapter exists yet.
    Games,
}

fn main() {
    let cli = Cli::parse();

    // Walking-skeleton default: an in-memory engine so `modlab games` is
    // runnable today. A real on-disk database at the configured data root
    // (F:\ModLab) lands once the store/transactions work begins.
    let core = Core::open_in_memory().expect("failed to start the ModLab engine");
    core.log_event("cli.invoked", "{}")
        .expect("failed to log invocation");

    match cli.command {
        Commands::Games => {
            println!("Aucun jeu détecté — aucun Game Adapter n'est encore implémenté.");
        }
    }
}
