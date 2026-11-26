//! Skyrim's fixed category list, and how a source label becomes one of them.
//!
//! The numbers are Nexus `category_id` values for Skyrim Special Edition.
//! `42` is User Interface: a local SkyUI `meta.ini` stores `nexusCategory=42`.
//! `-1` and `0` are MO2's "no category" and are not listed — the resolver
//! treats them as absent. English Nexus family names live in [`LABELS`], so
//! a stored name still translates when the number is one this table does
//! not know. An unknown number, with no name, stays empty.

use modlab_core::game_adapter::{
    category_step, match_category_name, CategoryHint, CategoryStep, GameCategory,
};
use std::path::Path;

pub const CATEGORIES: &[GameCategory] = &[
    GameCategory {
        id: "interface",
        label: "Interface",
        icon: "tab",
    },
    GameCategory {
        id: "gameplay",
        label: "Gameplay",
        icon: "sliders",
    },
    GameCategory {
        id: "combat",
        label: "Combat",
        icon: "zap",
    },
    GameCategory {
        id: "magic",
        label: "Magie",
        icon: "wand",
    },
    GameCategory {
        id: "equipment",
        label: "Armes et armures",
        icon: "shield",
    },
    GameCategory {
        id: "appearance",
        label: "Apparence",
        icon: "user",
    },
    GameCategory {
        id: "characters",
        label: "PNJ et compagnons",
        icon: "users",
    },
    GameCategory {
        id: "quests",
        label: "Quêtes et lieux",
        icon: "map",
    },
    GameCategory {
        id: "visuals",
        label: "Textures et visuels",
        icon: "image",
    },
    GameCategory {
        id: "environment",
        label: "Environnement",
        icon: "globe",
    },
    GameCategory {
        id: "audio",
        label: "Audio",
        icon: "disc",
    },
    GameCategory {
        id: "animation",
        label: "Animations",
        icon: "play",
    },
    GameCategory {
        id: "patches",
        label: "Patchs et correctifs",
        icon: "tool",
    },
    GameCategory {
        id: "utilities",
        label: "Utilitaires",
        icon: "gear",
    },
    GameCategory {
        id: "other",
        label: "Autre",
        icon: "box",
    },
];

/// Nexus Special Edition `category_id` → ModLab id.
///
/// Only numbers checked against a real file. `42` is User Interface: a
/// SkyUI `meta.ini` stores `nexusCategory=42`. An id that is not in this
/// table stays empty — a guessed number would file the mod in the wrong
/// family, which is worse than leaving the cell blank.
const NEXUS_SE: &[(i64, &str)] = &[(42, "interface")];

/// Normalized source labels. Keys are already folded (see
/// `normalize_category_label`). A longer parent wins for a child name
/// such as "Body, Face, and Hair - Body".
const LABELS: &[(&str, &str)] = &[
    ("interface", "interface"),
    ("user interface", "interface"),
    ("gameplay", "gameplay"),
    ("immersion", "gameplay"),
    ("stealth", "gameplay"),
    ("skills and leveling", "gameplay"),
    ("crafting", "gameplay"),
    ("overhauls", "gameplay"),
    ("guilds factions", "gameplay"),
    ("cheats and god items", "gameplay"),
    ("combat", "combat"),
    ("magic", "magic"),
    ("magie", "magic"),
    ("equipment", "equipment"),
    ("armes et armures", "equipment"),
    ("armour", "equipment"),
    ("armor", "equipment"),
    ("weapons", "equipment"),
    ("weapons and armour", "equipment"),
    ("weapons and armor", "equipment"),
    ("clothing", "equipment"),
    ("appearance", "appearance"),
    ("apparence", "appearance"),
    ("body face and hair", "appearance"),
    ("races classes and birthsigns", "appearance"),
    ("characters", "characters"),
    ("pnj et compagnons", "characters"),
    ("creatures and mounts", "characters"),
    ("npc", "characters"),
    ("npcs", "characters"),
    ("followers", "characters"),
    ("companions", "characters"),
    ("companion", "characters"),
    ("quests", "quests"),
    ("quetes et lieux", "quests"),
    ("buildings", "quests"),
    ("player homes", "quests"),
    ("quests and adventures", "quests"),
    ("locations", "quests"),
    ("collectibles treasure hunting and puzzles", "quests"),
    ("visuals", "visuals"),
    ("textures et visuels", "visuals"),
    ("visuals and graphics", "visuals"),
    ("models and textures", "visuals"),
    ("enb presets", "visuals"),
    ("enb preset", "visuals"),
    ("reshade", "visuals"),
    ("environment", "environment"),
    ("environnement", "environment"),
    ("environmental", "environment"),
    ("audio", "audio"),
    ("animation", "animation"),
    ("animations", "animation"),
    ("patches", "patches"),
    ("patchs et correctifs", "patches"),
    ("bug fixes", "patches"),
    ("utilities", "utilities"),
    ("utilitaires", "utilities"),
    ("modders resources", "utilities"),
    ("modder s resources", "utilities"),
    ("other", "other"),
    ("autre", "other"),
];

/// Whole-word tokens in a mod name. A phrase matches with word boundaries.
/// Two tokens that point at different families stay empty.
const NAME_TOKENS: &[(&str, &str)] = &[
    ("skyui", "interface"),
    ("ui", "interface"),
    ("hud", "interface"),
    ("menu", "interface"),
    ("skse", "utilities"),
    ("skse64", "utilities"),
    ("script extender", "utilities"),
    ("address library", "utilities"),
    ("papyrus util", "utilities"),
    ("papyrusutil", "utilities"),
    ("unofficial patch", "patches"),
    ("patch", "patches"),
    ("patches", "patches"),
    ("fix", "patches"),
    ("fixes", "patches"),
    ("fixed", "patches"),
    ("enb", "visuals"),
    ("reshade", "visuals"),
    ("texture", "visuals"),
    ("textures", "visuals"),
    ("hd", "visuals"),
    ("shader", "visuals"),
    ("shaders", "visuals"),
    ("smim", "visuals"),
    ("animation", "animation"),
    ("animations", "animation"),
    ("questline", "quests"),
    ("quest", "quests"),
    ("quests", "quests"),
    ("flora", "environment"),
    ("landscape", "environment"),
    ("weather", "environment"),
    ("music", "audio"),
    ("voice", "audio"),
    ("follower", "characters"),
    ("companion", "characters"),
    ("armor", "equipment"),
    ("armour", "equipment"),
    ("hair", "appearance"),
    ("spell", "magic"),
    ("spells", "magic"),
];

#[must_use]
pub fn resolve(hint: &CategoryHint<'_>) -> Option<&'static str> {
    let loaded = if hint.groups.is_empty() {
        fomod_group_names(hint.folder)
    } else {
        Vec::new()
    };
    let owned: Vec<&str> = if hint.groups.is_empty() {
        loaded.iter().map(String::as_str).collect()
    } else {
        hint.groups.to_vec()
    };
    let staged = CategoryHint {
        groups: &owned,
        folder: None,
        archive: None,
        ..*hint
    };
    match category_step(&staged, NEXUS_SE, LABELS) {
        CategoryStep::Settled(value) => value,
        CategoryStep::Open => {
            let from_name = hint
                .name
                .and_then(|name| match_category_name(name, LABELS, NAME_TOKENS));
            let (specific, skse_only) = content_signals(hint.folder, hint.archive);
            merge_fallback(from_name, specific, skse_only)
        }
    }
}

/// A name token wins over the generic "only an SKSE plugin" fallback.
/// A specific file signal (interface, animations, textures alone, audio
/// alone) that disagrees with the name stays empty.
fn merge_fallback(
    name: Option<&'static str>,
    specific: Option<&'static str>,
    skse_only: bool,
) -> Option<&'static str> {
    match (name, specific) {
        (Some(left), Some(right)) if left != right => None,
        (Some(id), _) => Some(id),
        (None, Some(id)) => Some(id),
        (None, None) if skse_only => Some("utilities"),
        (None, None) => None,
    }
}

fn content_signals(folder: Option<&Path>, archive: Option<&Path>) -> (Option<&'static str>, bool) {
    let mut names = Vec::new();
    let mut truncated = false;
    let mut seen_archive = false;
    for path in [folder, archive].into_iter().flatten() {
        if seen_archive && path.is_file() {
            continue;
        }
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            seen_archive = true;
        }
        let (found, cut) = modlab_core::archive::list_entry_names(path);
        truncated |= cut;
        names.extend(found);
    }
    classify_paths(&names, truncated)
}

fn classify_paths(paths: &[String], truncated: bool) -> (Option<&'static str>, bool) {
    if paths.is_empty() {
        return (None, false);
    }
    let interface = paths.iter().any(|path| interface_swf(path));
    let anim = paths.iter().any(|path| anim_path(path));
    let skse = paths.iter().any(|path| skse_plugin(path));
    let textures = paths.iter().any(|path| has_dir(path, "textures"));
    let meshes = paths.iter().any(|path| has_dir(path, "meshes"));
    let audio = paths
        .iter()
        .any(|path| has_dir(path, "sound") || has_dir(path, "music"));
    let plugin = paths.iter().any(|path| is_plugin(path));
    let scripts = paths.iter().any(|path| is_script(path));

    let mut specific = Vec::new();
    if interface {
        specific.push("interface");
    }
    if anim {
        specific.push("animation");
    }
    let alone = !interface && !anim && !skse && !plugin && !scripts;
    if !truncated && textures && !meshes && alone && !audio {
        specific.push("visuals");
    }
    if !truncated && audio && !textures && !meshes && alone {
        specific.push("audio");
    }
    let specific = match specific.as_slice() {
        [id] => Some(*id),
        _ => None,
    };
    let skse_only = !truncated
        && specific.is_none()
        && skse
        && !textures
        && !meshes
        && !anim
        && !interface
        && !audio
        && !plugin
        && !scripts;
    (specific, skse_only)
}

fn norm_entry(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn has_dir(path: &str, dir: &str) -> bool {
    let path = norm_entry(path);
    path.starts_with(&format!("{dir}/")) || path.contains(&format!("/{dir}/"))
}

fn interface_swf(path: &str) -> bool {
    let path = norm_entry(path);
    path.ends_with(".swf") && has_dir(&path, "interface")
}

fn anim_path(path: &str) -> bool {
    let path = norm_entry(path);
    path.starts_with("meshes/actors/character/animations/")
        || path.contains("/meshes/actors/character/animations/")
}

fn skse_plugin(path: &str) -> bool {
    let path = norm_entry(path);
    let in_plugins = path.starts_with("skse/plugins/") || path.contains("/skse/plugins/");
    in_plugins && (path.ends_with(".dll") || path.ends_with(".bin"))
}

fn is_plugin(path: &str) -> bool {
    let path = norm_entry(path);
    path.ends_with(".esp") || path.ends_with(".esm") || path.ends_with(".esl")
}

fn is_script(path: &str) -> bool {
    let path = norm_entry(path);
    has_dir(&path, "scripts") && (path.ends_with(".pex") || path.ends_with(".psc"))
}

fn fomod_group_names(folder: Option<&Path>) -> Vec<String> {
    let Some(folder) = folder else {
        return Vec::new();
    };
    let Some(path) = crate::fomod::find_module_config(folder) else {
        return Vec::new();
    };
    let Ok(module) = crate::fomod::read_module_config(&path) else {
        return Vec::new();
    };
    module
        .steps
        .into_iter()
        .flat_map(|step| step.groups.into_iter().map(|group| group.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn hint(raw: Option<&str>) -> CategoryHint<'_> {
        CategoryHint {
            raw,
            ..CategoryHint::default()
        }
    }

    #[test]
    fn the_list_is_short_and_ids_are_unique() {
        assert!(CATEGORIES.len() <= 15);
        assert!(CATEGORIES
            .iter()
            .any(|c| c.id == "other" && c.label == "Autre"));
        let mut ids: Vec<&str> = CATEGORIES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATEGORIES.len());
        for category in CATEGORIES {
            assert!(!category.id.is_empty());
            assert!(!category.label.is_empty());
            assert!(!category.icon.is_empty());
        }
    }

    #[test]
    fn nexus_ids_and_names_translate() {
        assert_eq!(
            resolve(&CategoryHint {
                nexus_id: Some(42),
                ..CategoryHint::default()
            }),
            Some("interface")
        );
        assert_eq!(resolve(&hint(Some("42"))), Some("interface"));
        assert_eq!(resolve(&hint(Some("User Interface"))), Some("interface"));
        assert_eq!(resolve(&hint(Some("Armour"))), Some("equipment"));
        assert_eq!(resolve(&hint(Some("Bug Fixes"))), Some("patches"));
        assert_eq!(
            resolve(&hint(Some("Magic - Spells & Enchantments"))),
            Some("magic")
        );
        assert_eq!(resolve(&hint(Some("ENB Presets"))), Some("visuals"));
        assert_eq!(resolve(&hint(Some("Modders Resources"))), Some("utilities"));
        assert_eq!(
            resolve(&hint(Some("Body, Face, and Hair - Hair"))),
            Some("appearance")
        );
    }

    #[test]
    fn unknown_or_ambiguous_stays_empty() {
        assert_eq!(
            resolve(&CategoryHint {
                nexus_id: Some(-1),
                ..CategoryHint::default()
            }),
            None
        );
        assert_eq!(
            resolve(&CategoryHint {
                nexus_id: Some(99999),
                ..CategoryHint::default()
            }),
            None
        );
        assert_eq!(resolve(&hint(Some("Miscellaneous"))), None);
        assert_eq!(resolve(&hint(Some("99999"))), None);
        assert_eq!(
            resolve(&CategoryHint {
                groups: &["Armour", "Magic"],
                ..CategoryHint::default()
            }),
            None
        );
        assert_eq!(
            resolve(&CategoryHint {
                raw: Some("Éclairage"),
                name: Some("Silent Horizons ENB"),
                ..CategoryHint::default()
            }),
            None,
            "a label we do not know is not overridden by the file name"
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("A random patch for cities"),
                ..CategoryHint::default()
            }),
            Some("patches")
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Northern Lights Overhaul"),
                ..CategoryHint::default()
            }),
            None,
            "a name with no clear token stays empty"
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Civil War Questline Fixed AF"),
                ..CategoryHint::default()
            }),
            None,
            "questline and fixed point at two families"
        );
    }

    #[test]
    fn groups_and_a_clear_name_are_a_last_resort() {
        assert_eq!(
            resolve(&CategoryHint {
                groups: &["Core", "User Interface"],
                ..CategoryHint::default()
            }),
            Some("interface")
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Silent Horizons ENB"),
                ..CategoryHint::default()
            }),
            Some("visuals")
        );
    }

    #[test]
    fn a_fomod_group_on_disk_counts_when_nothing_else_was_stored() {
        let dir = tempfile::tempdir().unwrap();
        let fomod = dir.path().join("fomod");
        fs::create_dir(&fomod).unwrap();
        fs::write(
            fomod.join("ModuleConfig.xml"),
            r#"<config>
                <moduleName>Example</moduleName>
                <installSteps>
                  <installStep name="Install">
                    <optionalFileGroups>
                      <group name="Audio" type="SelectExactlyOne">
                        <plugins><plugin name="On"><description/></plugin></plugins>
                      </group>
                    </optionalFileGroups>
                  </installStep>
                </installSteps>
              </config>"#,
        )
        .unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                folder: Some(dir.path()),
                ..CategoryHint::default()
            }),
            Some("audio")
        );
    }

    #[test]
    fn names_map_the_obvious_mods_and_leave_a_vague_one_empty() {
        assert_eq!(named("SkyUI"), Some("interface"));
        assert_eq!(named("SkyUI_SE"), Some("interface"));
        assert_eq!(named("SKSE"), Some("utilities"));
        assert_eq!(named("Skyrim Script Extender (SKSE64)"), Some("utilities"));
        assert_eq!(
            named("Address Library All in One (1.7.104.0) v13"),
            Some("utilities")
        );
        assert_eq!(named("PapyrusUtil"), Some("utilities"));
        assert_eq!(named("Engine Fixes"), Some("patches"));
        assert_eq!(
            named("Unofficial Skyrim Special Edition Patch"),
            Some("patches")
        );
        assert_eq!(named("Skyrim 2020 HD"), Some("visuals"));
        assert_eq!(named("SMIM SE 2-08"), Some("visuals"));
        assert_eq!(named("Open Animation Replacer"), Some("animation"));
        assert_eq!(named("A dragon overhaul"), None);
        assert_eq!(named("UI Patch"), None, "interface and patches both match");
    }

    #[test]
    fn archive_layout_fills_a_mod_whose_name_says_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let textures = dir.path().join("textures/actors");
        fs::create_dir_all(&textures).unwrap();
        fs::write(textures.join("skin.dds"), b"dds").unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Pretty Faces"),
                folder: Some(dir.path()),
                ..CategoryHint::default()
            }),
            Some("visuals")
        );

        let skse = tempfile::tempdir().unwrap();
        fs::create_dir_all(skse.path().join("SKSE/Plugins")).unwrap();
        fs::write(skse.path().join("SKSE/Plugins/versionlib.bin"), b"bin").unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Plugin Data"),
                folder: Some(skse.path()),
                ..CategoryHint::default()
            }),
            Some("utilities")
        );

        let ui = tempfile::tempdir().unwrap();
        fs::create_dir_all(ui.path().join("interface")).unwrap();
        fs::write(ui.path().join("interface/skyui.swf"), b"swf").unwrap();
        fs::write(ui.path().join("SkyUI_SE.esp"), b"esp").unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Inventory"),
                folder: Some(ui.path()),
                ..CategoryHint::default()
            }),
            Some("interface")
        );

        let anim = tempfile::tempdir().unwrap();
        let anim_dir = anim.path().join("meshes/actors/character/animations");
        fs::create_dir_all(&anim_dir).unwrap();
        fs::write(anim_dir.join("idle.hkx"), b"hkx").unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                folder: Some(anim.path()),
                ..CategoryHint::default()
            }),
            Some("animation")
        );
    }

    #[test]
    fn a_name_and_a_file_signal_that_disagree_stay_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("textures")).unwrap();
        fs::write(dir.path().join("textures/stone.dds"), b"dds").unwrap();
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Unofficial Patch"),
                folder: Some(dir.path()),
                ..CategoryHint::default()
            }),
            None
        );
    }

    fn named(name: &str) -> Option<&'static str> {
        resolve(&CategoryHint {
            name: Some(name),
            ..CategoryHint::default()
        })
    }
}
