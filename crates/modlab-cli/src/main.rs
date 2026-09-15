//! `modlab` — CLI client of the engine (`modlab-core`).
//!
//! Every command should eventually support `--json` so Claude Code can
//! drive this as efficiently as a human — see
//! docs/01-architecture-decidee.md §7.1.
//!
//! Data root: defaults to `.modlab-dev/` under the current directory (a
//! local sandbox, gitignored) rather than the real `F:\ModLab` — this CLI
//! is still a walking skeleton, not ready to touch real data yet. Pass
//! `--data-root` to point it elsewhere once that changes.

use clap::{Parser, Subcommand};
use modlab_core::{detect, loadout, store::Store, Core};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "modlab", version, about = "ModLab engine CLI")]
struct Cli {
    /// Root directory for the engine's data (db, store). Defaults to a
    /// local dev sandbox, never the real F:\ModLab, until the engine is
    /// further along.
    #[arg(long, default_value = ".modlab-dev")]
    data_root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect installed games. Currently Skyrim SE only, via a temporary
    /// path scan — see `modlab_core::detect` for why this isn't the real
    /// `GameAdapter` architecture yet.
    Detect,
    /// Import a file into the content-addressed store.
    ModImport {
        /// Path to the file to import.
        path: PathBuf,
    },
    /// Create a new, empty loadout.
    LoadoutCreate {
        /// Name of the loadout.
        name: String,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // A clean one-line message, not a Rust panic backtrace — this
            // is the boundary between the engine's Result-based errors and
            // a human (or Claude, via a future --json mode) reading stderr.
            eprintln!("modlab: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&cli.data_root)?;
    let db_path = cli.data_root.join("modlab.db");
    let core = Core::open(&db_path)?;
    core.log_event("cli.invoked", "{}")?;

    match cli.command {
        Commands::Detect => match detect::find_skyrim_special_edition() {
            Some(path) => println!("Skyrim Special Edition détecté : {}", path.display()),
            None => println!("Aucun jeu détecté."),
        },
        Commands::ModImport { path } => {
            let store = Store::open(cli.data_root.join("store").join("blobs"))?;
            let hash = store.put_file(&path)?;
            let payload = serde_json::json!({
                "source": path.display().to_string(),
                "hash": hash,
            })
            .to_string();
            core.log_event("mod.imported", &payload)?;
            println!("Importé dans le store : {hash}");
        }
        Commands::LoadoutCreate { name } => {
            let created = loadout::create(&core, &name)?;
            println!("Loadout créé : {}", created.name);
        }
    }
    Ok(())
}
