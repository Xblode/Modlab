//! Read-only parser for a Mod Organizer 2 profile directory
//! (`modlist.txt` / `plugins.txt` / `loadorder.txt`).
//!
//! This only *reads* — nothing here installs, deploys, or touches MO2's
//! `mods/` folder. `to_import_entries` bridges parsed data to ModLab's
//! generic import model (`modlab_core::loadout::ImportEntry`), but still
//! doesn't write anything — `loadout::import_mods` is what actually
//! persists, kept separate so parsing/mapping can be trusted independently
//! of the write path.
//!
//! Format notes, confirmed against a real profile rather than assumed:
//! - `modlist.txt`: one entry per line, `#`-comments and blank lines
//!   skipped. `+Name` / `-Name` is an MO2-managed mod (a real folder under
//!   `mods/`), enabled or disabled. `*Category: Name` is informational —
//!   Creation Club content, base-game DLC, or something MO2 found already
//!   sitting in `Data/` ("Unmanaged") — never a `mods/` folder, so callers
//!   must not try to look one up for these.
//! - `plugins.txt`: one plugin filename per line. A leading `*` means the
//!   plugin is active — this is the same convention the game's own native
//!   `plugins.txt` uses (MO2 doesn't invent its own here), so an unusual
//!   detail but not a guessed one.
//! - `loadorder.txt`: one plugin filename per line, already in load order.
//!
//! What's *not* confirmed: which direction `modlist.txt`'s line order maps
//! to MO2's on-screen priority (top-of-file vs. top-of-UI). Preserved
//! exactly as written for now; don't assume a direction until it's actually
//! needed for conflict resolution and can be checked against MO2's UI
//! directly.

use std::fs;
use std::io;
use std::path::Path;

use modlab_core::game_adapter::GameAdapter;
use modlab_core::{loadout, Core};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModlistEntry {
    /// A real MO2-managed mod — has a folder under `mods/`.
    Managed { name: String, enabled: bool },
    /// Tracked by MO2 but never a `mods/` folder: Creation Club content,
    /// base-game DLC, or pre-existing files it found in `Data/`.
    Informational { category: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEntry {
    pub filename: String,
    /// Whether the line carried the `*` active marker.
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mo2Profile {
    /// As written in `modlist.txt`, top to bottom.
    pub modlist: Vec<ModlistEntry>,
    /// As written in `plugins.txt`.
    pub plugins: Vec<PluginEntry>,
    /// As written in `loadorder.txt` — already in load order.
    pub load_order: Vec<String>,
}

impl Mo2Profile {
    /// Managed mod names only (skips DLC/CC/Unmanaged informational
    /// entries), in `modlist.txt` order.
    #[must_use]
    pub fn managed_mod_names(&self) -> Vec<&str> {
        self.modlist
            .iter()
            .filter_map(|e| match e {
                ModlistEntry::Managed { name, .. } => Some(name.as_str()),
                ModlistEntry::Informational { .. } => None,
            })
            .collect()
    }

    /// Convert the managed mods into ModLab's generic import shape, so
    /// `modlab_core::loadout::import_mods` can persist them without
    /// knowing anything about MO2. `mods_dir` is the instance's `mods/`
    /// folder (e.g. `<MO2 instance>/mods`) — each managed mod is expected
    /// to have a same-named subfolder there; informational entries
    /// (DLC/Creation Club/Unmanaged) are skipped since they were never a
    /// `mods/` folder to begin with.
    #[must_use]
    pub fn to_import_entries(&self, mods_dir: &Path) -> Vec<modlab_core::loadout::ImportEntry> {
        self.modlist
            .iter()
            .filter_map(|e| match e {
                ModlistEntry::Managed { name, enabled } => {
                    Some(modlab_core::loadout::ImportEntry {
                        name: name.clone(),
                        source_path: mods_dir.join(name),
                        enabled: *enabled,
                    })
                }
                ModlistEntry::Informational { .. } => None,
            })
            .collect()
    }
}

/// Import one MO2 profile into a new ModLab loadout: the whole read → map →
/// create → fill sequence that both the CLI and the UI shell execute, kept in
/// one place so they cannot drift apart.
///
/// Reads MO2's folders only — nothing is copied, and MO2's own files are
/// never modified. Returns the created loadout and how many mods landed in
/// it. Errors name what the caller can fix (missing profile, missing `mods/`
/// folder, or a profile whose managed mods match no folder — which would
/// otherwise silently import nothing).
pub fn import_into(
    core: &Core,
    profile_dir: &Path,
    mods_dir: &Path,
    loadout_name: &str,
) -> Result<(loadout::Loadout, usize), String> {
    let profile = read_profile(profile_dir).map_err(|e| e.to_string())?;
    let entries = profile.to_import_entries(mods_dir);
    if entries.is_empty() {
        return Err(format!(
            "aucun mod géré trouvé : les mods du profil {} ne correspondent à aucun \
             dossier de {}",
            profile_dir.display(),
            mods_dir.display()
        ));
    }
    // A modlist entry whose folder is gone must not silently become a mod
    // row pointing nowhere — that would be a loadout lying about files it
    // cannot deploy. Fail naming the offenders; the user fixes the profile
    // or the folder, nothing is half-imported.
    let missing: Vec<String> = entries
        .iter()
        .filter(|e| !e.source_path.is_dir())
        .map(|e| e.name.clone())
        .collect();
    if !missing.is_empty() {
        let shown: Vec<&str> = missing.iter().take(5).map(String::as_str).collect();
        let more = if missing.len() > shown.len() {
            format!(" et {} autres", missing.len() - shown.len())
        } else {
            String::new()
        };
        return Err(format!(
            "{} mod(s) listés dans le profil n'ont pas de dossier dans {} : {}{}",
            missing.len(),
            mods_dir.display(),
            shown.join(", "),
            more
        ));
    }
    let created = loadout::create(core, crate::SkyrimAdapter.id(), loadout_name)
        .map_err(|e| e.to_string())?;
    let imported = loadout::import_mods(core, created.id, &entries).map_err(|e| e.to_string())?;
    Ok((created, imported))
}

/// Read a full MO2 profile directory. Missing individual files are treated
/// as empty (a profile can genuinely have no plugins yet), but a missing
/// *directory* is a real error.
pub fn read_profile(profile_dir: &Path) -> io::Result<Mo2Profile> {
    if !profile_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("not a directory: {}", profile_dir.display()),
        ));
    }
    Ok(Mo2Profile {
        modlist: parse_modlist(&read_lines(&profile_dir.join("modlist.txt"))?),
        plugins: parse_plugins(&read_lines(&profile_dir.join("plugins.txt"))?),
        load_order: parse_load_order(&read_lines(&profile_dir.join("loadorder.txt"))?),
    })
}

fn read_lines(path: &Path) -> io::Result<Vec<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content.lines().map(str::to_string).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

fn is_meaningful(line: &str) -> bool {
    !line.trim().is_empty() && !line.starts_with('#')
}

// The if/else-if/else chain below reads more clearly than clippy's
// suggested nested `.map()`/`.map_or_else()` chain would for a 4-way
// prefix dispatch — kept as-is deliberately.
#[allow(clippy::manual_map)]
fn parse_modlist(lines: &[String]) -> Vec<ModlistEntry> {
    lines
        .iter()
        .filter(|l| is_meaningful(l))
        .filter_map(|line| {
            if let Some(name) = line.strip_prefix('+') {
                Some(ModlistEntry::Managed {
                    name: name.to_string(),
                    enabled: true,
                })
            } else if let Some(name) = line.strip_prefix('-') {
                Some(ModlistEntry::Managed {
                    name: name.to_string(),
                    enabled: false,
                })
            } else if let Some(rest) = line.strip_prefix('*') {
                Some(rest.split_once(": ").map_or_else(
                    || ModlistEntry::Informational {
                        category: String::new(),
                        name: rest.to_string(),
                    },
                    |(category, name)| ModlistEntry::Informational {
                        category: category.to_string(),
                        name: name.to_string(),
                    },
                ))
            } else {
                // Unrecognized line shape — skip rather than guess. Real
                // MO2 output only ever uses +/-/* prefixes for entries.
                None
            }
        })
        .collect()
}

fn parse_plugins(lines: &[String]) -> Vec<PluginEntry> {
    lines
        .iter()
        .filter(|l| is_meaningful(l))
        .map(|line| match line.strip_prefix('*') {
            Some(filename) => PluginEntry {
                filename: filename.to_string(),
                active: true,
            },
            None => PluginEntry {
                filename: line.clone(),
                active: false,
            },
        })
        .collect()
}

fn parse_load_order(lines: &[String]) -> Vec<String> {
    lines.iter().filter(|l| is_meaningful(l)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn profile_with(modlist: &str, plugins: &str, load_order: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("modlist.txt"), modlist).unwrap();
        fs::write(dir.path().join("plugins.txt"), plugins).unwrap();
        fs::write(dir.path().join("loadorder.txt"), load_order).unwrap();
        dir
    }

    /// A minimal but realistic instance: two managed mods (one enabled, one
    /// disabled), one informational DLC line, real mod folders for the
    /// managed names only.
    fn fake_instance() -> (tempfile::TempDir, tempfile::TempDir) {
        let profile = profile_with(
            "# This file was automatically generated by Mod Organizer.\n\
             +SkyUI\n\
             -Address Library All in One\n\
             *DLC: Dawnguard\n",
            "*SkyUI_SE.esp\nSkyUI_SE.esp\n",
            "SkyUI_SE.esp\n",
        );
        let mods = tempfile::tempdir().unwrap();
        fs::create_dir_all(mods.path().join("SkyUI")).unwrap();
        fs::create_dir_all(mods.path().join("Address Library All in One")).unwrap();
        fs::write(mods.path().join("SkyUI/SkyUI_SE.esp"), b"x").unwrap();
        (profile, mods)
    }

    #[test]
    fn import_into_creates_a_skyrim_loadout_from_a_profile() {
        let core = Core::open_in_memory().unwrap();
        let (profile, mods) = fake_instance();

        let (created, imported) =
            import_into(&core, profile.path(), mods.path(), "Main 2026").unwrap();

        assert_eq!(created.name, "Main 2026");
        assert_eq!(imported, 2, "the informational DLC line is skipped");

        let listed = loadout::list(&core).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].game_id, "skyrimse");
        assert_eq!(listed[0].name, "Main 2026");

        // Read back through the Core to assert what was actually stored,
        // in modlist order with the enabled flag preserved.
        let rows = loadout::entries(&core, created.id).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "SkyUI");
        assert!(rows[0].enabled);
        assert_eq!(rows[1].name, "Address Library All in One");
        assert!(!rows[1].enabled);
        assert_eq!(
            rows[0].source_path,
            mods.path().join("SkyUI"),
            "mods point at MO2's folders in place — never copied"
        );
    }

    #[test]
    fn import_into_rejects_a_profile_whose_mods_match_no_folder() {
        let core = Core::open_in_memory().unwrap();
        let profile = profile_with("+Ghost Mod\n+Real Mod\n", "", "");
        let mods = tempfile::tempdir().unwrap();
        fs::create_dir_all(mods.path().join("Real Mod")).unwrap();

        let error = import_into(&core, profile.path(), mods.path(), "Vide").unwrap_err();

        assert!(
            error.contains("Ghost Mod") && error.contains("n'ont pas de dossier"),
            "the error names the mod whose folder is gone: {error}"
        );
        assert!(
            loadout::list(&core).unwrap().is_empty(),
            "no loadout must be created when a mod would point nowhere"
        );
    }

    #[test]
    fn import_into_names_a_missing_profile_directory() {
        let core = Core::open_in_memory().unwrap();
        let nowhere = tempfile::tempdir().unwrap().path().join("gone");

        let error = import_into(&core, &nowhere, Path::new("nowhere"), "X").unwrap_err();

        assert!(error.contains("gone"), "the message names the bad path");
    }

    #[test]
    fn parses_a_realistic_profile() {
        // Mirrors a real profile's shape (Benjamin's own "Default" MO2
        // profile), not invented syntax.
        let dir = profile_with(
            "# This file was automatically generated by Mod Organizer.\n\
             +SkyUI\n\
             +Address Library All in One\n\
             *Creation Club: ccBGSSSE001-Fish\n\
             *DLC: Dawnguard\n\
             *Unmanaged: 144fps\n",
            "# This file was automatically generated by Mod Organizer.\n\
             unofficial skyrim special edition patch.esp\n\
             144fps.esp\n\
             *SkyUI_SE.esp\n",
            "# This file was automatically generated by Mod Organizer.\n\
             Skyrim.esm\n\
             Update.esm\n\
             SkyUI_SE.esp\n",
        );

        let profile = read_profile(dir.path()).unwrap();

        assert_eq!(
            profile.modlist,
            vec![
                ModlistEntry::Managed {
                    name: "SkyUI".into(),
                    enabled: true
                },
                ModlistEntry::Managed {
                    name: "Address Library All in One".into(),
                    enabled: true
                },
                ModlistEntry::Informational {
                    category: "Creation Club".into(),
                    name: "ccBGSSSE001-Fish".into(),
                },
                ModlistEntry::Informational {
                    category: "DLC".into(),
                    name: "Dawnguard".into()
                },
                ModlistEntry::Informational {
                    category: "Unmanaged".into(),
                    name: "144fps".into(),
                },
            ]
        );
        assert_eq!(
            profile.managed_mod_names(),
            vec!["SkyUI", "Address Library All in One"]
        );

        assert_eq!(
            profile.plugins,
            vec![
                PluginEntry {
                    filename: "unofficial skyrim special edition patch.esp".into(),
                    active: false
                },
                PluginEntry {
                    filename: "144fps.esp".into(),
                    active: false
                },
                PluginEntry {
                    filename: "SkyUI_SE.esp".into(),
                    active: true
                },
            ]
        );

        assert_eq!(
            profile.load_order,
            vec!["Skyrim.esm", "Update.esm", "SkyUI_SE.esp"]
        );
    }

    #[test]
    fn disabled_managed_mod_is_parsed_correctly() {
        let dir = profile_with("-Some Disabled Mod\n", "", "");
        let profile = read_profile(dir.path()).unwrap();
        assert_eq!(
            profile.modlist,
            vec![ModlistEntry::Managed {
                name: "Some Disabled Mod".into(),
                enabled: false
            }]
        );
    }

    #[test]
    fn missing_files_are_treated_as_empty_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        // No modlist.txt/plugins.txt/loadorder.txt written at all.
        let profile = read_profile(dir.path()).unwrap();
        assert_eq!(profile, Mo2Profile::default());
    }

    #[test]
    fn missing_directory_is_a_real_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_profile(&dir.path().join("does-not-exist"));
        assert!(result.is_err());
    }

    #[test]
    fn reads_benjamins_real_default_profile_if_present() {
        // Bonus real-world check, same spirit as the adapter's own
        // machine-detection test — skipped gracefully if this machine
        // doesn't have it (e.g. CI, or a future different dev machine).
        let real_profile = Path::new("F:/_Modding/Skyrim Modding/Mod Organazer/profiles/Default");
        if !real_profile.is_dir() {
            return;
        }
        let profile = read_profile(real_profile).unwrap();
        assert!(
            !profile.modlist.is_empty(),
            "expected at least the known managed mods (SkyUI, Address Library)"
        );
        assert!(profile.managed_mod_names().contains(&"SkyUI"));
    }

    #[test]
    fn converts_managed_mods_to_import_entries_and_skips_informational() {
        let dir = profile_with("+SkyUI\n-Disabled Mod\n*DLC: Dawnguard\n", "", "");
        let profile = read_profile(dir.path()).unwrap();
        let mods_dir = Path::new("F:/instance/mods");

        let entries = profile.to_import_entries(mods_dir);

        assert_eq!(
            entries,
            vec![
                modlab_core::loadout::ImportEntry {
                    name: "SkyUI".into(),
                    source_path: mods_dir.join("SkyUI"),
                    enabled: true,
                },
                modlab_core::loadout::ImportEntry {
                    name: "Disabled Mod".into(),
                    source_path: mods_dir.join("Disabled Mod"),
                    enabled: false,
                },
            ],
            "DLC entry must not appear — it was never a mods/ folder"
        );
    }
}
