//! Integration test against the repo's `fixtures/fake-skyrim/` — never the
//! real game, never F:\ModLab. See fixtures/README.md for what's real vs.
//! placeholder in there.

use modlab_core::store::Store;
use std::path::PathBuf;

fn fixture_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("fake-skyrim")
        .join(rel)
}

#[test]
fn imports_a_fixture_plugin_into_the_store() {
    let store_dir = tempfile::tempdir().unwrap();
    let store = Store::open(store_dir.path()).unwrap();

    let plugin = fixture_path("Data/Fake Plugin.esp");
    assert!(plugin.is_file(), "fixture is missing: {}", plugin.display());

    let hash = store.put_file(&plugin).unwrap();
    assert!(store.contains(&hash));
}

#[test]
fn fixture_has_the_expected_shape_for_detection() {
    assert!(fixture_path("SkyrimSE.exe").is_file());
    assert!(fixture_path("Data").is_dir());
    assert!(fixture_path("Data/SKSE/Plugins/FakePlugin.dll").is_file());
}
