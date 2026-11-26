//! Virtual loadout row for the branch Minecraft loader (Fabric, Forge, …).
//!
//! The loader lives under `versions/` and `libraries/` — not in `mods/`. Rows
//! point at `modlab:branch-mc-loader:…` so the table can badge the tool
//! without importing library jars.

use modlab_core::game_adapter::ClassifiedTool;
use std::path::Path;

/// Virtual `source_path` prefix — not a disk path.
pub const SOURCE_PREFIX: &str = "modlab:branch-mc-loader:";

pub fn source_token(loader: &str, mc: &str, loader_ver: Option<&str>) -> String {
    let loader = loader.trim().to_lowercase();
    let mc = mc.trim();
    let lv = loader_ver
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    format!("{SOURCE_PREFIX}{loader}/{mc}/{lv}")
}

/// Parse a stored branch-loader token. `None` when this is an ordinary path.
pub fn parse_source_token(path: &str) -> Option<(&str, &str, Option<&str>)> {
    let rest = path.strip_prefix(SOURCE_PREFIX)?;
    let (loader, tail) = rest.split_once('/')?;
    let (mc, lv) = tail.split_once('/')?;
    let loader_ver = match lv {
        "-" => None,
        other => Some(other),
    };
    Some((loader, mc, loader_ver))
}

pub fn classify_token(query_path: &Path) -> Option<ClassifiedTool> {
    let text = query_path.to_str()?;
    let (loader, _, loader_ver) = parse_source_token(text)?;
    Some(ClassifiedTool {
        tool: loader.to_string(),
        version: loader_ver.map(str::to_string),
    })
}

#[must_use]
pub fn display_name(loader: &str) -> &'static str {
    match loader.trim().to_ascii_lowercase().as_str() {
        "fabric" => "Fabric Loader",
        "quilt" => "Quilt Loader",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "vanilla" => "Minecraft",
        _ => "Loader",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trip_and_classify() {
        let token = source_token("fabric", "1.20.1", Some("0.15.11"));
        assert_eq!(
            parse_source_token(&token),
            Some(("fabric", "1.20.1", Some("0.15.11")))
        );
        let tool = classify_token(Path::new(&token)).unwrap();
        assert_eq!(tool.tool, "fabric");
        assert_eq!(tool.version.as_deref(), Some("0.15.11"));
    }
}
