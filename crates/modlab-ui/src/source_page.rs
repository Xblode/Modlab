//! Host and page link for a download the human already started.
//!
//! Both strings are addresses the embedded browser already had: the open
//! page, and the file URL the download event reported. Nothing here sends
//! a request or reads HTML. A mod id is kept only when it is already in
//! the page URL, so the Info panel can reopen that page.

/// What to store on the mod row. `source_host` is one of `nexus`,
/// `curseforge`, `modrinth`, `silverlock`, `other`, `unknown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOrigin {
    pub source_host: &'static str,
    pub page_url: Option<String>,
}

/// `page` is the open page (`None` when the browser had no real page).
/// `file_url` is the download itself.
#[must_use]
pub fn capture_origin(page: Option<&str>, file_url: &str) -> CaptureOrigin {
    let page = page.and_then(http_url);
    let file = http_url(file_url);
    let page_kind = page.as_ref().and_then(|url| host_kind(url));
    let file_kind = file.as_ref().and_then(|url| host_kind(url));
    let source_host = page_kind
        .or(file_kind)
        .unwrap_or(if page.is_some() || file.is_some() {
            "other"
        } else {
            "unknown"
        });
    let page_url = page
        .as_ref()
        .and_then(canonical_page)
        .or_else(|| {
            page.as_ref()
                .and_then(|url| generic_page(url, file.as_ref()))
        })
        .or_else(|| file.as_ref().and_then(canonical_page));
    CaptureOrigin {
        source_host,
        page_url,
    }
}

fn http_url(raw: &str) -> Option<url::Url> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("about:blank") {
        return None;
    }
    let parsed = url::Url::parse(raw).ok()?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return None;
    }
    Some(parsed)
}

fn host_kind(url: &url::Url) -> Option<&'static str> {
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host == "nexusmods.com"
        || host.ends_with(".nexusmods.com")
        || host == "nexus-cdn.com"
        || host.ends_with(".nexus-cdn.com")
    {
        return Some("nexus");
    }
    if host == "curseforge.com"
        || host.ends_with(".curseforge.com")
        || host == "forgecdn.net"
        || host.ends_with(".forgecdn.net")
    {
        return Some("curseforge");
    }
    if host == "modrinth.com" || host.ends_with(".modrinth.com") {
        return Some("modrinth");
    }
    if host == "silverlock.org" || host.ends_with(".silverlock.org") {
        return Some("silverlock");
    }
    None
}

fn segments(url: &url::Url) -> Vec<String> {
    url.path_segments()
        .map(|parts| {
            parts
                .filter(|part| !part.is_empty())
                .map(|part| part.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn canonical_page(url: &url::Url) -> Option<String> {
    match host_kind(url) {
        Some("nexus") => nexus_page(url),
        Some("curseforge") => curseforge_page(url),
        Some("modrinth") => modrinth_page(url),
        _ => None,
    }
}

fn nexus_page(url: &url::Url) -> Option<String> {
    let parts = segments(url);
    if parts.len() < 3 || !parts[1].eq_ignore_ascii_case("mods") {
        return None;
    }
    if !parts[2].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!(
        "https://www.nexusmods.com/{}/mods/{}",
        parts[0], parts[2]
    ))
}

fn curseforge_page(url: &url::Url) -> Option<String> {
    let parts = segments(url);
    if parts.len() < 3 || parts[0].eq_ignore_ascii_case("api") {
        return None;
    }
    let slug = &parts[2];
    if slug.eq_ignore_ascii_case("search") || is_archive_name(slug) {
        return None;
    }
    Some(format!(
        "https://www.curseforge.com/{}/{}/{}",
        parts[0], parts[1], slug
    ))
}

fn modrinth_page(url: &url::Url) -> Option<String> {
    const KINDS: &[&str] = &[
        "mod",
        "modpack",
        "plugin",
        "datapack",
        "resourcepack",
        "shader",
    ];
    let parts = segments(url);
    if parts.len() < 2 {
        return None;
    }
    if !KINDS.iter().any(|kind| parts[0].eq_ignore_ascii_case(kind)) {
        return None;
    }
    let slug = &parts[1];
    if slug.eq_ignore_ascii_case("version") || is_archive_name(slug) {
        return None;
    }
    Some(format!("https://modrinth.com/{}/{}", parts[0], slug))
}

fn generic_page(page: &url::Url, file: Option<&url::Url>) -> Option<String> {
    if is_archive_name(page.path()) {
        return None;
    }
    let cleaned = without_query(page);
    if let Some(file) = file {
        if cleaned == without_query(file) {
            return None;
        }
    }
    Some(cleaned)
}

fn without_query(url: &url::Url) -> String {
    let mut cleaned = url.clone();
    cleaned.set_query(None);
    cleaned.set_fragment(None);
    cleaned.to_string()
}

fn is_archive_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".zip", ".jar", ".7z", ".rar", ".exe", ".msi"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nexus_page_and_a_cdn_file_keep_the_mod_page() {
        let origin = capture_origin(
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/123?tab=files"),
            "https://cf-files.nexusmods.com/cdn/123/lux.7z",
        );
        assert_eq!(origin.source_host, "nexus");
        assert_eq!(
            origin.page_url.as_deref(),
            Some("https://www.nexusmods.com/skyrimspecialedition/mods/123")
        );
    }

    #[test]
    fn curseforge_and_modrinth_pages_drop_the_file_tail() {
        let curse = capture_origin(
            Some("https://www.curseforge.com/minecraft/mc-mods/jei/files/6000900"),
            "https://edge.forgecdn.net/files/6000/900/jei.jar",
        );
        assert_eq!(curse.source_host, "curseforge");
        assert_eq!(
            curse.page_url.as_deref(),
            Some("https://www.curseforge.com/minecraft/mc-mods/jei")
        );

        let modrinth = capture_origin(
            Some("https://modrinth.com/mod/sodium/version/mc1.16.5-0.2.0"),
            "https://cdn.modrinth.com/data/abc/versions/def/sodium.jar",
        );
        assert_eq!(modrinth.source_host, "modrinth");
        assert_eq!(
            modrinth.page_url.as_deref(),
            Some("https://modrinth.com/mod/sodium")
        );
    }

    #[test]
    fn silverlock_keeps_the_open_page_and_a_cdn_alone_has_no_page() {
        let silver = capture_origin(
            Some("https://skse.silverlock.org/"),
            "https://skse.silverlock.org/skse64.7z",
        );
        assert_eq!(silver.source_host, "silverlock");
        assert_eq!(
            silver.page_url.as_deref(),
            Some("https://skse.silverlock.org/")
        );

        let cdn = capture_origin(
            None,
            "https://cdn.modrinth.com/data/abc/versions/def/sodium.jar",
        );
        assert_eq!(cdn.source_host, "modrinth");
        assert_eq!(cdn.page_url, None);

        let blank = capture_origin(Some("about:blank"), "https://example.com/notes.7z");
        assert_eq!(blank.source_host, "other");
        assert_eq!(blank.page_url, None);

        let none = capture_origin(None, "");
        assert_eq!(none.source_host, "unknown");
        assert_eq!(none.page_url, None);
    }
}
