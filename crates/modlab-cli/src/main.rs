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
use modlab_core::{archive, conflicts, loadout, store::Store, transaction, Core};
use modlab_game_minecraft::{instance as mc_instance, MinecraftAdapter};
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
    /// List a Minecraft instance's `mods/` folder: each jar, the version its
    /// own manifest declares, and the loader they agree on. Read-only.
    McScan {
        /// Path to the instance root (the folder holding `mods/`).
        instance_root: PathBuf,
    },
    /// Import a Minecraft instance's `mods/` folder into a new loadout.
    /// Read-only on the game's side — not a jar is moved or renamed.
    McImport {
        /// Path to the instance root (the folder holding `mods/`).
        instance_root: PathBuf,
        /// Name for the new ModLab loadout.
        loadout_name: String,
    },
    /// Copy a downloaded `.jar` into an instance's `mods/` folder and add it
    /// to a loadout. A real write into the game folder: the copy is staged
    /// then renamed, and an existing jar of the same name is never
    /// overwritten.
    McInstall {
        /// Loadout to add the mod to.
        loadout_id: i64,
        /// Path to the `.jar` to install.
        jar: PathBuf,
        /// Path to the instance root (the folder holding `mods/`).
        instance_root: PathBuf,
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
    /// Virtually deploy a loadout's enabled mods and launch the real game
    /// through `skse64_loader.exe`, hooked, so it sees the merged view.
    /// This is the real thing, not a check: it opens a real, visible game
    /// window. Blocks until the whole session (loader + game + anything
    /// they spawn) has exited, then disconnects the VFS.
    SkseLaunch {
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
        Commands::Detect => {
            let found: Vec<_> = [SkyrimAdapter.detect(), MinecraftAdapter.detect()]
                .into_iter()
                .flatten()
                .collect();
            if found.is_empty() {
                println!("Aucun jeu détecté.");
            } else {
                for install in found {
                    println!("{} détecté : {}", install.name, install.root.display());
                }
            }
        }
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
            let (created, imported) = mo2::import_into(&core, &profile, &mods_dir, &loadout_name)?;
            println!(
                "Loadout '{}' (id {}) créé avec {imported} mod(s) importé(s) depuis {}",
                created.name,
                created.id,
                mods_dir.display()
            );
        }
        Commands::McScan { instance_root } => {
            let scanned = mc_instance::scan_dir(&mc_instance::mods_dir(&instance_root))?;
            println!(
                "Loader : {} — {} mod(s) dans {}",
                scanned.loader.id(),
                scanned.mods.len(),
                mc_instance::mods_dir(&instance_root).display()
            );
            for m in &scanned.mods {
                println!(
                    "  [{}] {} {} ({})",
                    if m.enabled { "x" } else { " " },
                    m.name,
                    m.version.as_deref().unwrap_or("version inconnue"),
                    m.loader.id()
                );
            }
        }
        Commands::McImport {
            instance_root,
            loadout_name,
        } => {
            let (created, imported) =
                mc_instance::import_instance(&core, &instance_root, &loadout_name)?;
            println!(
                "Loadout '{}' (id {}) créé avec {imported} mod(s) importé(s) depuis {}",
                created.name,
                created.id,
                mc_instance::mods_dir(&instance_root).display()
            );
        }
        Commands::McInstall {
            loadout_id,
            jar,
            instance_root,
        } => {
            let installed = mc_instance::install_jar(&core, loadout_id, &jar, &instance_root)?;
            println!(
                "'{}' {} installé dans {}",
                installed.name,
                installed.version.as_deref().unwrap_or("(version inconnue)"),
                installed.jar_path.display()
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

            let vfs_instance_name = usvfs::instance_name("skyrim", loadout_id);
            let mut vfs = usvfs::Vfs::create(&usvfs_dll, &vfs_instance_name)?;
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

            vfs.disconnect()?;
        }
        Commands::SkseLaunch {
            usvfs_dll,
            loadout_id,
        } => {
            let install = SkyrimAdapter
                .detect()
                .ok_or("Skyrim SE introuvable sur cette machine")?;
            let data_dir = install.root.join("Data");
            let loader = install.root.join("skse64_loader.exe");
            if !loader.is_file() {
                return Err(format!(
                    "skse64_loader.exe introuvable dans {}",
                    install.root.display()
                )
                .into());
            }
            let rows = loadout::entries(&core, loadout_id)?;

            // Deterministic per-loadout name — never the shared "modlab"
            // literal — so two loadouts launched independently can't
            // reconnect to the same VFS session (see
            // usvfs::instance_name's doc).
            let vfs_instance_name = usvfs::instance_name("skyrim", loadout_id);
            let mut vfs = usvfs::Vfs::create(&usvfs_dll, &vfs_instance_name)?;
            let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
            tx.step(format!("usvfs instance '{vfs_instance_name}' connected"));
            tx.commit()?;
            let linked = usvfs::deploy_enabled_mods(&vfs, &rows, &data_dir)?;
            println!(
                "{linked} mod(s) déployé(s) virtuellement sur {}.",
                data_dir.display()
            );

            let cmd = format!("\"{}\"", loader.display());
            println!("Lancement de {cmd}...");
            vfs.launch_hooked_visible(&cmd, &install.root)?;

            println!(
                "Jeu lancé. En attente de la fin de la session (ferme Skyrim pour terminer)..."
            );
            vfs.wait_for_session_end(Duration::from_secs(2), Duration::from_secs(60))?;

            // Explicit, verified disconnect rather than leaving it to
            // `Drop`: a silently failed teardown here would leave a stale
            // session for the next launch of this same loadout to collide
            // with.
            let disconnect_result = vfs.disconnect();
            let step = match &disconnect_result {
                Ok(()) => format!("usvfs instance '{vfs_instance_name}' disconnected (confirmed)"),
                Err(e) => format!("usvfs instance '{vfs_instance_name}' disconnect FAILED: {e}"),
            };
            let mut tx = transaction::Transaction::new(&core, "skyrim.usvfs");
            tx.step(step);
            tx.commit()?;
            disconnect_result?;
            println!("Session terminée, VFS déconnectée.");
        }
        Commands::LoadoutConflicts { loadout_id } => {
            let rows = loadout::entries(&core, loadout_id)?;
            // Same scan the UI badges its rows from — one implementation, two
            // clients (see `conflicts::report_for_entries`).
            let report = conflicts::report_for_entries(&rows);
            for (mod_id, error) in &report.unreadable {
                println!("  ! mod {mod_id} non analysé — {error}");
            }

            if report.conflicts.is_empty() {
                if report.unreadable.is_empty() {
                    println!("Aucun conflit de fichier entre les mods activés.");
                } else {
                    // Saying "no conflict" while some mods were never read
                    // would be a lie, not a summary.
                    println!(
                        "Aucun conflit entre les mods analysés — mais {} mod(s) n'ont pas pu \
                         être lus (voir ci-dessus).",
                        report.unreadable.len()
                    );
                }
            } else {
                println!(
                    "{} fichier(s) en conflit (ordre de priorité = ordre d'import, \
                     pas encore confirmé contre l'UI MO2) :",
                    report.conflicts.len()
                );
                for conflict in &report.conflicts {
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
