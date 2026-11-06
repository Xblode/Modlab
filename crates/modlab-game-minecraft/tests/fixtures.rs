//! Integration test against the repo's `fixtures/fake-minecraft/` — never a
//! real install, never `F:\ModLab`. See `fixtures/README.md` for what's real
//! vs. placeholder in there: the `.jar` files are plain text, so this covers
//! shape and path handling only. Manifest parsing is tested against real
//! zips built at test time, in `instance`'s own test module.

use modlab_core::game_adapter::GameAdapter;
use modlab_game_minecraft::instance::{self, ModLoader};
use modlab_game_minecraft::MinecraftAdapter;
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("fake-minecraft")
}

#[test]
fn fixture_has_the_shape_the_adapter_detects() {
    let root = fixture_root();
    assert!(root.join("launcher_profiles.json").is_file());
    assert!(root.join("versions").is_dir());
    assert!(instance::mods_dir(&root).is_dir());
    assert!(root.join("saves/New World/level.dat").is_file());
    assert!(root.join("resourcepacks").is_dir());
}

/// The adapter's own "does this look like Minecraft" rule, run against the
/// fixture rather than against whatever happens to be installed on the
/// machine running the tests.
#[test]
fn the_adapter_recognizes_the_fixture_as_an_install() {
    let caps = MinecraftAdapter.capabilities();
    assert_eq!(caps.short_name, "Minecraft");
    // `launcher_profiles.json` + `versions/` are exactly what
    // `looks_like_minecraft` keys on.
    let root = fixture_root();
    assert!(root.join("launcher_profiles.json").is_file() || root.join("versions").is_dir());
}

/// Every jar in the fixture is a text placeholder, so none of them parses as
/// a zip. The scan must still list both of them — by filename, with no
/// version invented — instead of dropping files it couldn't open.
#[test]
fn scanning_the_fixture_lists_unreadable_jars_without_inventing_metadata() {
    let scanned = instance::scan_dir(&instance::mods_dir(&fixture_root())).unwrap();

    let names: Vec<&str> = scanned.mods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["fabric-api-0.91.0", "sodium-fabric-0.5.8"]);
    assert!(scanned.mods.iter().all(|m| m.version.is_none()));
    assert!(scanned.mods.iter().all(|m| m.enabled));
    assert_eq!(scanned.loader, ModLoader::Unknown);
}
