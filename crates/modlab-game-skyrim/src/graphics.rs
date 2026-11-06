//! A minimal `SkyrimPrefs.ini` editor for the dashboard's "Graphismes" quick
//! action — just the handful of `[Display]` keys that action exposes, not a
//! general-purpose INI parser. `write` only ever changes the lines for those
//! known keys (or appends them to `[Display]` if the file never had them);
//! every other line, section and key — including ones this module doesn't
//! understand — passes through byte-for-byte untouched.

use std::fs;
use std::io;
use std::path::Path;

const SECTION: &str = "[Display]";
const KEYS: [&str; 6] = [
    "iSize W",
    "iSize H",
    "bFull Screen",
    "iPresentInterval",
    "iMultiSample",
    "bFXAAEnabled",
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GraphicsSettings {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub vsync: Option<bool>,
    /// `iMultiSample` — 0, 2, 4 or 8 in practice, stored as whatever the
    /// file actually has rather than validated against that list.
    pub antialiasing: Option<u32>,
    pub fxaa: Option<bool>,
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn render_bool(v: bool) -> &'static str {
    if v {
        "1"
    } else {
        "0"
    }
}

/// Read the `[Display]` keys this module knows about. A missing file, or a
/// file with no `[Display]` section (or an individual key missing from it),
/// just leaves the corresponding field `None` — never an error, since "not
/// configured yet" is a normal state for a fresh install.
pub fn read(path: &Path) -> io::Result<GraphicsSettings> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(GraphicsSettings::default()),
        Err(e) => return Err(e),
    };
    let mut settings = GraphicsSettings::default();
    let mut in_display = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_display = trimmed.eq_ignore_ascii_case(SECTION);
            continue;
        }
        if !in_display {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "iSize W" => settings.width = value.parse().ok(),
            "iSize H" => settings.height = value.parse().ok(),
            "bFull Screen" => settings.fullscreen = parse_bool(value),
            "iPresentInterval" => settings.vsync = parse_bool(value),
            "iMultiSample" => settings.antialiasing = value.parse().ok(),
            "bFXAAEnabled" => settings.fxaa = parse_bool(value),
            _ => {}
        }
    }
    Ok(settings)
}

fn render_key(key: &str, s: &GraphicsSettings) -> Option<String> {
    match key {
        "iSize W" => s.width.map(|v| format!("iSize W={v}")),
        "iSize H" => s.height.map(|v| format!("iSize H={v}")),
        "bFull Screen" => s.fullscreen.map(|v| format!("bFull Screen={}", render_bool(v))),
        "iPresentInterval" => s.vsync.map(|v| format!("iPresentInterval={}", render_bool(v))),
        "iMultiSample" => s.antialiasing.map(|v| format!("iMultiSample={v}")),
        "bFXAAEnabled" => s.fxaa.map(|v| format!("bFXAAEnabled={}", render_bool(v))),
        _ => None,
    }
}

/// Write `new`'s fields into `[Display]`, creating the file (and the
/// section, and any of these keys it doesn't already have) if needed. A
/// field left `None` leaves that key's existing line untouched rather than
/// removing it — this is a partial update, not a replace.
pub fn write(path: &Path, new: &GraphicsSettings) -> io::Result<()> {
    let existing = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };

    let mut out: Vec<String> = Vec::new();
    let mut written = [false; KEYS.len()];
    let mut in_display = false;
    let mut saw_display_section = false;

    let append_missing = |out: &mut Vec<String>, written: &mut [bool; KEYS.len()]| {
        for (i, key) in KEYS.iter().enumerate() {
            if !written[i] {
                if let Some(rendered) = render_key(key, new) {
                    out.push(rendered);
                }
                written[i] = true;
            }
        }
    };

    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_display {
                append_missing(&mut out, &mut written);
            }
            in_display = trimmed.eq_ignore_ascii_case(SECTION);
            if in_display {
                saw_display_section = true;
            }
            out.push(line.to_string());
            continue;
        }
        if in_display {
            if let Some((key, _)) = trimmed.split_once('=') {
                if let Some(i) = KEYS.iter().position(|k| *k == key.trim()) {
                    if let Some(rendered) = render_key(KEYS[i], new) {
                        written[i] = true;
                        out.push(rendered);
                        continue;
                    }
                }
            }
        }
        out.push(line.to_string());
    }
    if in_display {
        append_missing(&mut out, &mut written);
    }
    if !saw_display_section {
        out.push(SECTION.to_string());
        append_missing(&mut out, &mut written);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, out.join("\r\n") + "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_known_display_keys_and_ignores_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SkyrimPrefs.ini");
        fs::write(
            &path,
            "[General]\r\nsLanguage=fr\r\n\r\n[Display]\r\niSize W=2560\r\niSize H=1440\r\nbFull Screen=0\r\niPresentInterval=1\r\niMultiSample=4\r\nbFXAAEnabled=1\r\nfSomeUnknownKnob=0.5\r\n\r\n[Audio]\r\nfMasterVolume=1\r\n",
        )
        .unwrap();

        let s = read(&path).unwrap();
        assert_eq!(s.width, Some(2560));
        assert_eq!(s.height, Some(1440));
        assert_eq!(s.fullscreen, Some(false));
        assert_eq!(s.vsync, Some(true));
        assert_eq!(s.antialiasing, Some(4));
        assert_eq!(s.fxaa, Some(true));
    }

    #[test]
    fn missing_file_reads_as_all_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SkyrimPrefs.ini");
        assert_eq!(read(&path).unwrap(), GraphicsSettings::default());
    }

    #[test]
    fn write_updates_only_the_targeted_keys_and_preserves_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SkyrimPrefs.ini");
        fs::write(
            &path,
            "[General]\r\nsLanguage=fr\r\n\r\n[Display]\r\niSize W=1920\r\niSize H=1080\r\nbFull Screen=1\r\nfSomeUnknownKnob=0.5\r\n\r\n[Audio]\r\nfMasterVolume=1\r\n",
        )
        .unwrap();

        write(
            &path,
            &GraphicsSettings {
                width: Some(2560),
                height: Some(1440),
                fullscreen: None,
                vsync: Some(true),
                antialiasing: Some(8),
                fxaa: Some(false),
            },
        )
        .unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("sLanguage=fr"), "{text}");
        assert!(text.contains("iSize W=2560"), "{text}");
        assert!(text.contains("iSize H=1440"), "{text}");
        assert!(text.contains("bFull Screen=1"), "left untouched: {text}");
        assert!(text.contains("fSomeUnknownKnob=0.5"), "{text}");
        assert!(text.contains("fMasterVolume=1"), "{text}");
        assert!(text.contains("iPresentInterval=1"), "{text}");
        assert!(text.contains("iMultiSample=8"), "{text}");
        assert!(text.contains("bFXAAEnabled=0"), "{text}");

        let read_back = read(&path).unwrap();
        assert_eq!(read_back.width, Some(2560));
        assert_eq!(read_back.fullscreen, Some(true), "untouched value read back correctly");
    }

    #[test]
    fn write_creates_the_file_and_section_when_neither_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("SkyrimPrefs.ini");

        write(
            &path,
            &GraphicsSettings {
                width: Some(1920),
                height: Some(1080),
                fullscreen: Some(true),
                vsync: Some(false),
                antialiasing: Some(0),
                fxaa: Some(false),
            },
        )
        .unwrap();

        let s = read(&path).unwrap();
        assert_eq!(
            s,
            GraphicsSettings {
                width: Some(1920),
                height: Some(1080),
                fullscreen: Some(true),
                vsync: Some(false),
                antialiasing: Some(0),
                fxaa: Some(false),
            }
        );
    }
}
