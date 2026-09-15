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
use modlab_core::game_adapter::GameAdapter;
use modlab_core::{archive, conflicts, loadout, store::Store, Core};
use modlab_game_skyrim::{installer, mo2, usvfs, SkyrimAdapter};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

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
    /// Detect installed games via their Game Adapter. Skyrim SE only so
    /// far (`modlab_game_skyrim::SkyrimAdapter`).
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
    /// Read an MO2 profile directory and print what was found. Read-only —
    /// does not import or touch anything.
    Mo2Profile {
        /// Path to the MO2 profile directory (contains modlist.txt etc.).
        path: PathBuf,
    },
    /// Import an MO2 instance's managed mods into a new ModLab loadout.
    /// Only records what ModLab knows about each mod (name, source path,
    /// enabled state) — does not copy files into the store or touch MO2's
    /// own folders in any way.
    Mo2Import {
        /// Path to the MO2 profile directory (contains modlist.txt etc.).
        profile: PathBuf,
        /// Path to the MO2 instance's `mods/` folder.
        mods_dir: PathBuf,
        /// Name for the new ModLab loadout.
        loadout_name: String,
    },
    /// Extract an archive to a staging area and report what shape it is
    /// (bare Data content, wrapped Data/ folder, FOMOD, unrecognized).
    /// Doesn't install anything — inspection only.
    ArchiveInspect {
        /// Path to the .7z or .zip archive.
        path: PathBuf,
    },
    /// Find files provided by more than one enabled mod in a loadout.
    /// Priority order used is the loadout's stored import/add order — see
    /// `modlab_core::conflicts` for why "who wins" isn't asserted beyond
    /// that until MO2's real priority direction is confirmed.
    LoadoutConflicts {
        /// Id of the loadout to check (printed by `mo2-import`/`loadout-create`).
        loadout_id: i64,
    },
    /// Virtually deploy a loadout's enabled mods onto the detected Skyrim
    /// install's real `Data/` folder via USVFS, then run a read-only
    /// directory listing through the hooked view to prove it worked.
    /// Nothing on real disk is read, written, or moved — USVFS only
    /// intercepts file APIs inside the one hooked `cmd.exe` this spawns,
    /// and that command never writes inside `Data/`. Does not launch
    /// Skyrim or SKSE — that's a separate, still-unapproved step.
    UsvfsCheck {
        /// Path to `usvfs_x64.dll` (e.g. from an existing MO2 install).
        usvfs_dll: PathBuf,
        /// Loadout whose enabled mods should be virtually deployed.
        loadout_id: i64,
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
        Commands::Detect => match SkyrimAdapter.detect() {
            Some(install) => println!("{} détecté : {}", install.name, install.root.display()),
            None => println!("Aucun jeu détecté."),
        },
        Commands::Mo2Profile { path } => {
            let profile = mo2::read_profile(&path)?;
            println!(
                "{} mods gérés, {} entrées informatives (DLC/CC/non géré), {} plugins, {} dans l'ordre de chargement",
                profile.managed_mod_names().len(),
                profile.modlist.len() - profile.managed_mod_names().len(),
                profile.plugins.len(),
                profile.load_order.len(),
            );
            for name in profile.managed_mod_names() {
                println!("  + {name}");
            }
        }
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
            let created = loadout::create(&core, SkyrimAdapter.id(), &name)?;
            println!("Loadout créé : {} (id {})", created.name, created.id);
        }
        Commands::Mo2Import {
            profile,
            mods_dir,
            loadout_name,
        } => {
            let profile = mo2::read_profile(&profile)?;
            let entries = profile.to_import_entries(&mods_dir);
            let created = loadout::create(&core, SkyrimAdapter.id(), &loadout_name)?;
            let imported = loadout::import_mods(&core, created.id, &entries)?;
            println!(
                "Loadout '{}' (id {}) créé avec {imported} mod(s) importé(s) depuis {}",
                created.name,
                created.id,
                mods_dir.display()
            );
        }
        Commands::ArchiveInspect { path } => {
            let staging = cli
                .data_root
                .join("staging")
                .join(path.file_stem().unwrap_or_default());
            archive::extract(&path, &staging)?;
            let detected = installer::detect_shape(&staging)?;
            println!(
                "{}: {:?} (contenu réel dans {})",
                path.display(),
                detected.shape,
                detected.content_root.display()
            );
        }
        Commands::UsvfsCheck {
            usvfs_dll,
            loadout_id,
        } => {
            let install = SkyrimAdapter
                .detect()
                .ok_or("Skyrim SE introuvable sur cette machine")?;
            let data_dir = install.root.join("Data");
            let rows = loadout::entries(&core, loadout_id)?;

            let vfs = usvfs::Vfs::create(&usvfs_dll, "modlab")?;
            let linked = usvfs::deploy_enabled_mods(&vfs, &rows, &data_dir)?;

            let staging = cli.data_root.join("staging");
            std::fs::create_dir_all(&staging)?;
            let out_file = staging.join("usvfs-check.txt");
            let cmd = format!(
                "cmd.exe /c dir /b \"{}\" > \"{}\" 2>&1",
                data_dir.display(),
                out_file.display()
            );
            let exit_code = vfs.run_hooked(&cmd, Duration::from_secs(15))?;
            let listing = std::fs::read_to_string(&out_file)?;

            println!(
                "{linked} mod(s) lié(s) virtuellement sur {} (lecture seule — rien écrit sur le vrai disque, jeu non lancé).",
                data_dir.display()
            );
            println!("Commande de vérification terminée avec le code {exit_code}.");
            println!(
                "Aperçu du contenu vu à travers la VFS ({} entrée(s) au total) :",
                listing.lines().count()
            );
            for line in listing.lines().take(20) {
                println!("  {line}");
            }
        }
        Commands::LoadoutConflicts { loadout_id } => {
            let rows = loadout::entries(&core, loadout_id)?;
            let mods_files = rows
                .iter()
                .filter(|r| r.enabled)
                .map(|r| {
                    Ok(conflicts::ModFiles {
                        mod_id: r.mod_id,
                        name: r.name.clone(),
                        files: conflicts::list_files_relative(&r.source_path)?,
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?;

            let found = conflicts::find_conflicts(&mods_files);
            if found.is_empty() {
                println!("Aucun conflit de fichier entre les mods activés.");
            } else {
                println!(
                    "{} fichier(s) en conflit (ordre de priorité = ordre d'import, \
                     pas encore confirmé contre l'UI MO2) :",
                    found.len()
                );
                for conflict in &found {
                    let contributors: Vec<_> = conflict
                        .contributors
                        .iter()
                        .map(|(_, name)| name.as_str())
                        .collect();
                    println!(
                        "  {} — {} (gagnant : {})",
                        conflict.relative_path.display(),
                        contributors.join(" < "),
                        conflict.winner().1
                    );
                }
            }
        }
    }
    Ok(())
}
