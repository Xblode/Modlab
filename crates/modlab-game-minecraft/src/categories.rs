//! Minecraft's fixed category list, and how a CurseForge or Modrinth
//! label becomes one of them.
//!
//! Labels are matched after folding (case, accents, hyphens). A string
//! that is not in the table stays empty — "Food" or "Miscellaneous" is
//! not forced into a nearby family.

use modlab_core::game_adapter::{
    category_step, match_category_name, CategoryHint, CategoryStep, GameCategory,
};

pub const CATEGORIES: &[GameCategory] = &[
    GameCategory {
        id: "library",
        label: "Bibliothèques et API",
        icon: "layers",
    },
    GameCategory {
        id: "performance",
        label: "Performance",
        icon: "zap",
    },
    GameCategory {
        id: "technology",
        label: "Technologie",
        icon: "gear",
    },
    GameCategory {
        id: "magic",
        label: "Magie",
        icon: "wand",
    },
    GameCategory {
        id: "adventure",
        label: "Aventure et RPG",
        icon: "flag",
    },
    GameCategory {
        id: "world",
        label: "Monde",
        icon: "globe",
    },
    GameCategory {
        id: "mobs",
        label: "Créatures",
        icon: "users",
    },
    GameCategory {
        id: "storage",
        label: "Stockage",
        icon: "box",
    },
    GameCategory {
        id: "decoration",
        label: "Décoration",
        icon: "palette",
    },
    GameCategory {
        id: "interface",
        label: "Interface",
        icon: "tab",
    },
    GameCategory {
        id: "utilities",
        label: "Utilitaires",
        icon: "tool",
    },
    GameCategory {
        id: "visuals",
        label: "Shaders et visuels",
        icon: "sparkle",
    },
    GameCategory {
        id: "other",
        label: "Autre",
        icon: "more",
    },
];

const LABELS: &[(&str, &str)] = &[
    ("library", "library"),
    ("bibliotheques et api", "library"),
    ("api and library", "library"),
    ("library and api", "library"),
    ("performance", "performance"),
    ("optimization", "performance"),
    ("technology", "technology"),
    ("technologie", "technology"),
    ("redstone", "technology"),
    ("automation", "technology"),
    ("processing", "technology"),
    ("energy", "technology"),
    ("magic", "magic"),
    ("magie", "magic"),
    ("adventure", "adventure"),
    ("aventure et rpg", "adventure"),
    ("adventure and rpg", "adventure"),
    ("adventure rpg", "adventure"),
    ("world", "world"),
    ("monde", "world"),
    ("world gen", "world"),
    ("world generation", "world"),
    ("worldgen", "world"),
    ("biomes", "world"),
    ("dimensions", "world"),
    ("structures", "world"),
    ("mobs", "mobs"),
    ("creatures", "mobs"),
    ("storage", "storage"),
    ("stockage", "storage"),
    ("decoration", "decoration"),
    ("cosmetic", "decoration"),
    ("interface", "interface"),
    ("map and information", "interface"),
    ("map information", "interface"),
    ("utilities", "utilities"),
    ("utilitaires", "utilities"),
    ("utility", "utilities"),
    ("utility and qol", "utilities"),
    ("utility qol", "utilities"),
    ("server utility", "utilities"),
    ("management", "utilities"),
    ("bug fixes", "utilities"),
    ("visuals", "visuals"),
    ("shaders et visuels", "visuals"),
    ("shaders", "visuals"),
    ("shader", "visuals"),
    ("other", "other"),
    ("autre", "other"),
];

/// Tokens safe in a mod name or a mod id. `core` / `api` / `lib` are too
/// common in a free-text description, so they are not in [`DESCRIPTION_TOKENS`].
const NAME_TOKENS: &[(&str, &str)] = &[
    ("api", "library"),
    ("lib", "library"),
    ("library", "library"),
    ("core", "library"),
    ("sodium", "performance"),
    ("lithium", "performance"),
    ("ferritecore", "performance"),
    ("ferrite core", "performance"),
    ("modernfix", "performance"),
    ("modern fix", "performance"),
    ("optifine", "performance"),
    ("iris", "visuals"),
    ("jei", "interface"),
    ("rei", "interface"),
    ("emi", "interface"),
    ("journeymap", "interface"),
    ("journey map", "interface"),
    ("xaero", "interface"),
    ("xaerominimap", "interface"),
    ("xaeros minimap", "interface"),
    ("minimap", "interface"),
    ("shader", "visuals"),
    ("shaders", "visuals"),
    ("create", "technology"),
    ("jade", "interface"),
    ("appleskin", "interface"),
    ("apple skin", "interface"),
    ("mouse tweaks", "utilities"),
    ("mousetweaks", "utilities"),
    ("modmenu", "interface"),
    ("mod menu", "interface"),
    ("cloth config", "library"),
    ("clothconfig", "library"),
    ("architectury", "library"),
    ("geckolib", "library"),
    ("gecko lib", "library"),
    ("terrablender", "world"),
    ("terra blender", "world"),
    ("biomes o plenty", "world"),
    ("biomesoplenty", "world"),
    ("waystones", "adventure"),
    ("farmers delight", "technology"),
    ("farmer s delight", "technology"),
    ("farmersdelight", "technology"),
    ("iron chests", "storage"),
    ("ironchests", "storage"),
    ("sophisticated backpacks", "storage"),
    ("sophisticatedbackpacks", "storage"),
    ("botania", "magic"),
    ("mekanism", "technology"),
    ("ae2", "technology"),
    ("applied energistics", "technology"),
    ("thermal", "technology"),
    ("tinkers", "technology"),
    ("tinkers construct", "technology"),
    ("tconstruct", "technology"),
    ("twilight forest", "adventure"),
    ("twilightforest", "adventure"),
    ("alexs mobs", "mobs"),
    ("alex s mobs", "mobs"),
    ("alexsmobs", "mobs"),
    ("oculus", "visuals"),
    ("embeddium", "performance"),
    ("rubidium", "performance"),
    ("entity culling", "performance"),
    ("entityculling", "performance"),
    ("starlight", "performance"),
    ("c2me", "performance"),
    ("better end", "world"),
    ("betterend", "world"),
];

const DESCRIPTION_TOKENS: &[(&str, &str)] = &[
    ("sodium", "performance"),
    ("lithium", "performance"),
    ("ferritecore", "performance"),
    ("ferrite core", "performance"),
    ("modernfix", "performance"),
    ("modern fix", "performance"),
    ("optifine", "performance"),
    ("iris", "visuals"),
    ("jei", "interface"),
    ("rei", "interface"),
    ("emi", "interface"),
    ("journeymap", "interface"),
    ("journey map", "interface"),
    ("xaero", "interface"),
    ("minimap", "interface"),
    ("shader", "visuals"),
    ("shaders", "visuals"),
    ("mekanism", "technology"),
    ("botania", "magic"),
    ("biomes o plenty", "world"),
    ("farmers delight", "technology"),
    ("farmer s delight", "technology"),
    ("twilight forest", "adventure"),
    ("embeddium", "performance"),
    ("rubidium", "performance"),
    ("geckolib", "library"),
    ("architectury", "library"),
    ("cloth config", "library"),
    ("mod menu", "interface"),
    ("apple skin", "interface"),
];

/// Mod ids with punctuation stripped. Only mods whose family is obvious.
const KNOWN_MODS: &[(&str, &str)] = &[
    ("fabricapi", "library"),
    ("sodium", "performance"),
    ("lithium", "performance"),
    ("ferritecore", "performance"),
    ("modernfix", "performance"),
    ("optifine", "performance"),
    ("iris", "visuals"),
    ("jei", "interface"),
    ("rei", "interface"),
    ("emi", "interface"),
    ("journeymap", "interface"),
    ("xaerominimap", "interface"),
    ("xaerosminimap", "interface"),
    ("xaeroworldmap", "interface"),
    ("create", "technology"),
    ("jade", "interface"),
    ("appleskin", "interface"),
    ("mousetweaks", "utilities"),
    ("modmenu", "interface"),
    ("clothconfig", "library"),
    ("architectury", "library"),
    ("geckolib", "library"),
    ("terrablender", "world"),
    ("biomesoplenty", "world"),
    ("waystones", "adventure"),
    ("farmersdelight", "technology"),
    ("ironchests", "storage"),
    ("sophisticatedbackpacks", "storage"),
    ("botania", "magic"),
    ("mekanism", "technology"),
    ("ae2", "technology"),
    ("appliedenergistics2", "technology"),
    ("thermal", "technology"),
    ("thermalfoundation", "technology"),
    ("thermalexpansion", "technology"),
    ("tconstruct", "technology"),
    ("tinkersconstruct", "technology"),
    ("twilightforest", "adventure"),
    ("alexsmobs", "mobs"),
    ("oculus", "visuals"),
    ("embeddium", "performance"),
    ("rubidium", "performance"),
    ("entityculling", "performance"),
    ("starlight", "performance"),
    ("c2me", "performance"),
    ("betterend", "world"),
];

#[must_use]
pub fn resolve(hint: &CategoryHint<'_>) -> Option<&'static str> {
    match category_step(hint, &[], LABELS) {
        CategoryStep::Settled(value) => value,
        CategoryStep::Open => infer_open(hint),
    }
}

fn infer_open(hint: &CategoryHint<'_>) -> Option<&'static str> {
    let mut clues = crate::archive_meta::CategoryClues::default();
    for path in [hint.folder, hint.archive].into_iter().flatten() {
        clues.absorb(crate::archive_meta::category_clues(path));
    }
    let from_ids = vote_ids(&clues.mod_ids);
    let from_badge = if clues.library_badge {
        Vote::Hit("library")
    } else {
        Vote::Empty
    };
    let mut names: Vec<&str> = clues.names.iter().map(String::as_str).collect();
    if let Some(name) = hint.name {
        names.push(name);
    }
    for path in [hint.folder, hint.archive].into_iter().flatten() {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            names.push(stem);
        }
    }
    let from_names = vote_names(&names, NAME_TOKENS);
    let strong = from_ids.join(from_badge).join(from_names);
    match strong {
        Vote::Hit(id) => Some(id),
        Vote::Clash => None,
        Vote::Empty => clues
            .description
            .as_deref()
            .and_then(|text| match_category_name(text, LABELS, DESCRIPTION_TOKENS)),
    }
}

fn vote_ids(ids: &[String]) -> Vote {
    let mut vote = Vote::Empty;
    for id in ids {
        let key = norm_mod_id(id);
        if let Some((_, category)) = KNOWN_MODS.iter().find(|(known, _)| *known == key) {
            vote = vote.push(category);
        }
    }
    vote
}

fn vote_names(names: &[&str], tokens: &[(&'static str, &'static str)]) -> Vote {
    let mut vote = Vote::Empty;
    for name in names {
        if let Some(category) = match_category_name(name, LABELS, tokens) {
            vote = vote.push(category);
        }
    }
    vote
}

fn norm_mod_id(id: &str) -> String {
    id.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[derive(Clone, Copy)]
enum Vote {
    Empty,
    Hit(&'static str),
    Clash,
}

impl Vote {
    fn push(self, id: &'static str) -> Self {
        match self {
            Self::Empty => Self::Hit(id),
            Self::Hit(prev) if prev == id => Self::Hit(prev),
            Self::Hit(_) | Self::Clash => Self::Clash,
        }
    }

    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Clash, _) | (_, Self::Clash) => Self::Clash,
            (Self::Empty, other) => other,
            (Self::Hit(id), Self::Empty) => Self::Hit(id),
            (Self::Hit(left), Self::Hit(right)) if left == right => Self::Hit(left),
            (Self::Hit(_), Self::Hit(_)) => Self::Clash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hint(raw: Option<&str>) -> CategoryHint<'_> {
        CategoryHint {
            raw,
            ..CategoryHint::default()
        }
    }

    #[test]
    fn the_list_exists_and_ids_are_unique() {
        assert!(!CATEGORIES.is_empty());
        assert!(CATEGORIES
            .iter()
            .any(|c| c.id == "other" && c.label == "Autre"));
        let mut ids: Vec<&str> = CATEGORIES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATEGORIES.len());
        for category in CATEGORIES {
            assert!(!category.icon.is_empty());
        }
    }

    #[test]
    fn curseforge_and_modrinth_labels_translate() {
        assert_eq!(resolve(&hint(Some("Technology"))), Some("technology"));
        assert_eq!(resolve(&hint(Some("API and Library"))), Some("library"));
        assert_eq!(resolve(&hint(Some("api-and-library"))), Some("library"));
        assert_eq!(resolve(&hint(Some("optimization"))), Some("performance"));
        assert_eq!(resolve(&hint(Some("adventure-rpg"))), Some("adventure"));
        assert_eq!(resolve(&hint(Some("Adventure and RPG"))), Some("adventure"));
        assert_eq!(resolve(&hint(Some("worldgen"))), Some("world"));
        assert_eq!(resolve(&hint(Some("World Gen"))), Some("world"));
        assert_eq!(resolve(&hint(Some("Cosmetic"))), Some("decoration"));
        assert_eq!(
            resolve(&hint(Some("Map and Information"))),
            Some("interface")
        );
        assert_eq!(resolve(&hint(Some("Utility & QoL"))), Some("utilities"));
        assert_eq!(resolve(&hint(Some("library"))), Some("library"));
    }

    #[test]
    fn unknown_labels_stay_empty() {
        assert_eq!(resolve(&hint(Some("Food"))), None);
        assert_eq!(resolve(&hint(Some("Miscellaneous"))), None);
        assert_eq!(resolve(&hint(Some("Addons"))), None);
        assert_eq!(resolve(&hint(Some("potato"))), None);
        assert_eq!(
            resolve(&CategoryHint {
                nexus_id: Some(42),
                ..CategoryHint::default()
            }),
            None,
            "a Skyrim Nexus id is not a Minecraft category"
        );
    }

    #[test]
    fn a_shader_in_the_name_is_visuals_and_a_vague_name_is_not() {
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Complementary Shaders"),
                ..CategoryHint::default()
            }),
            Some("visuals")
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("Just Another Mod"),
                ..CategoryHint::default()
            }),
            None
        );
    }

    #[test]
    fn known_names_and_mod_ids_fill_the_obvious_families() {
        assert_eq!(named("Sodium"), Some("performance"));
        assert_eq!(named("Lithium"), Some("performance"));
        assert_eq!(named("FerriteCore"), Some("performance"));
        assert_eq!(named("ModernFix"), Some("performance"));
        assert_eq!(named("OptiFine"), Some("performance"));
        assert_eq!(named("Iris"), Some("visuals"));
        assert_eq!(named("jei-26.3-neoforge-31.14.0.72"), Some("interface"));
        assert_eq!(named("Fabric API"), Some("library"));
        assert_eq!(named("JourneyMap"), Some("interface"));
        assert_eq!(named("REI"), Some("interface"));
        assert_eq!(named("Create"), Some("technology"));
        assert_eq!(named("Mekanism"), Some("technology"));
        assert_eq!(named("Botania"), Some("magic"));
        assert_eq!(named("AppleSkin"), Some("interface"));
        assert_eq!(named("Farmer's Delight"), Some("technology"));
        assert_eq!(named("Biomes O' Plenty"), Some("world"));
        assert_eq!(named("Alex's Mobs"), Some("mobs"));
        assert_eq!(named("Embeddium"), Some("performance"));
        assert_eq!(named("Better End"), Some("world"));
        assert_eq!(named("Just Another Mod"), None);
        assert_eq!(
            named("Sodium Shaders"),
            None,
            "performance and visuals both match"
        );
    }

    #[test]
    fn a_fabric_api_jar_is_a_library_even_with_a_dull_file_name() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("mod.jar");
        write_jar(
            &jar,
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"fabric-api","name":"Fabric API","description":"Core API module","custom":{"modmenu":{"badges":["library"]}}}"#,
        );
        assert_eq!(
            resolve(&CategoryHint {
                name: Some("mod"),
                archive: Some(&jar),
                ..CategoryHint::default()
            }),
            Some("library")
        );
    }

    #[test]
    fn a_jei_toml_and_a_shader_description_are_read_locally() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("view.jar");
        write_jar(
            &jar,
            "META-INF/mods.toml",
            "[[mods]]\nmodId=\"jei\"\ndisplayName=\"Just Enough Items\"\ndescription=\"Recipe viewer\"\n",
        );
        assert_eq!(
            resolve(&CategoryHint {
                archive: Some(&jar),
                ..CategoryHint::default()
            }),
            Some("interface")
        );

        assert_eq!(named("Pack"), None);
        let described = dir.path().join("described.jar");
        write_jar(
            &described,
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"examplepack","name":"Pack","description":"A shader pack"}"#,
        );
        assert_eq!(
            resolve(&CategoryHint {
                archive: Some(&described),
                ..CategoryHint::default()
            }),
            Some("visuals")
        );
    }

    #[test]
    fn a_description_that_only_says_core_does_not_become_a_library() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("food.jar");
        write_jar(
            &jar,
            "fabric.mod.json",
            r#"{"schemaVersion":1,"id":"examplefood","name":"Example Food","description":"The core cooking loop, with new meals"}"#,
        );
        assert_eq!(
            resolve(&CategoryHint {
                archive: Some(&jar),
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

    fn write_jar(path: &std::path::Path, entry: &str, body: &str) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        use std::io::Write;
        zip.write_all(body.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
}
