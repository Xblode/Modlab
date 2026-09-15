//! Read-only parser for a Mod Organizer 2 profile directory
//! (`modlist.txt` / `plugins.txt` / `loadorder.txt`).
//!
//! This only *reads* — nothing here installs, deploys, or touches MO2's
//! `mods/` folder. That's the next step (see the "[SKYRIM] Import d'instance
//! MO2" ClickUp task), deliberately kept separate so parsing can be trusted
//! on its own before anything acts on what it finds.
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
}
