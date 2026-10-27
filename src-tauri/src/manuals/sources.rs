//! Where manuals come from, in order: Retrokit's hand-catalogued manuals, then
//! the Internet Archive's own "Console Manuals" collections (Kirkland's scans
//! among them). Both live on the Internet Archive. Each platform's list of
//! names is fetched once and cached for a month; a manual itself downloads
//! only when someone opens it.
//!
//! Internet Archive rules for apps (archive.org/developers/bots.html, read
//! 2026-10-27): a descriptive User-Agent, cache, few requests at a time, honor
//! 429. No account needed for public items.

use std::sync::OnceLock;

use regex::Regex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::art::http::Fetch;
use crate::art::matcher::{self, Index};
use crate::db::queries;
use crate::error::{Error, Result};

/// Lists of names are fetched again after this long.
pub const LIST_MAX_AGE_SECS: i64 = 30 * 24 * 3600;

/// Retrokit's folder for each KB platform (archive.org/details/retrokit-manuals).
pub fn retrokit_system(platform: &str) -> Option<&'static str> {
    Some(match platform {
        "snes" => "snes",
        "n64" => "n64",
        "gba" => "gba",
        "nds" => "nds",
        "segacd" => "segacd",
        "saturn" => "saturn",
        "ps1" => "psx",
        "psp" => "psp",
        _ => return None,
    })
}

/// The Internet Archive's "Console Manuals" sets for each KB platform, best first.
pub fn ia_collections(platform: &str) -> &'static [&'static str] {
    match platform {
        "ps1" => &["playstationmanuals"],
        "ps2" => &["ps2-kirklands-manual-labor", "sonyplaystation2manuals"],
        "snes" => &["snes-kirklands-manual-labor", "manuals_snes"],
        "n64" => &["nintendo64manuals"],
        "gba" => &["gameboyadvancemanuals", "tabor62gbamanual"],
        "saturn" => &["segasaturnmanuals"],
        "segacd" => &["megacdmanuals"],
        "xbox" => &["xboxmanuals"],
        "xbox360" => &["xbox360manuals"],
        _ => &[],
    }
}

/// One manual a list offers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// "Title (Region)", what the matcher reads.
    pub name: String,
    /// A direct https download (Retrokit) or "ia:<identifier>" (resolved when downloaded).
    pub url: String,
    /// What the picker shows.
    pub label: String,
    pub size: Option<u64>,
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("valid regex"))
}

/// Region a Retrokit language list stands for: English → USA (its "en" is the
/// American release), Japanese → Japan, anything else → Europe.
fn region_of_langs(langs: &str) -> &'static str {
    let langs: Vec<&str> = langs.split(',').map(str::trim).collect();
    if langs.iter().any(|l| *l == "en" || *l == "en-us" || *l == "en-ca") {
        "USA"
    } else if langs.contains(&"ja") {
        "Japan"
    } else {
        "Europe"
    }
}

/// Retrokit's zip listing (archive.org's "view archive" page): one row per manual.
pub fn parse_retrokit(html: &str) -> Vec<Entry> {
    static ROW: OnceLock<Regex> = OnceLock::new();
    static LANGS: OnceLock<Regex> = OnceLock::new();
    let row = re(&ROW, r#"<a href="(//archive\.org/download/retrokit-manuals/[^"]+)">([^<]+)</a><td><td>[^<]*<td id="size">(\d+)"#);
    let langs = re(&LANGS, r"^(.*?)\s*\(([a-z]{2}(?:-[a-z]{2})?(?:,[a-z]{2}(?:-[a-z]{2})?)*)\)$");
    row.captures_iter(html)
        .filter_map(|c| {
            let file = html_unescape(&c[2]);
            let stem = file.strip_suffix(".pdf")?;
            let (title, region) = match langs.captures(stem) {
                Some(l) => (l[1].to_string(), region_of_langs(&l[2])),
                None => (stem.to_string(), "USA"),
            };
            Some(Entry {
                name: format!("{title} ({region})"),
                url: format!("https:{}", &c[1]),
                label: format!("Retrokit · {stem}"),
                size: c[3].parse().ok(),
            })
        })
        .collect()
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&#039;", "'").replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">")
}

/// Words that say what an Internet Archive item is rather than which game: taken off the start and end of titles.
const NOISE: &[&str] = &[
    "instruction manual", "instruction booklet", "instructions", "instruction", "manuals", "manual", "booklet", "user guide",
    "guide", "scans", "scan", "packaging and", "packaging", "box", "cart", "cover", "disc", "spinecard", "hiresscans", "ocr",
    "color", "colour", "xbox 360", "x360", "xbox", "nintendo 64", "n64", "game boy advance", "gameboy advance", "gba",
    "super nintendo", "super famicom", "snes", "sfc", "playstation 2", "playstation", "ps2", "ps1", "psx", "psone", "sega saturn",
    "saturn", "sega mega-cd", "sega cd", "mega-cd", "mega cd", "megacd", "sony", "pal", "ntsc", "english", "en de fr es it",
    "en fr de es it", "and",
];

/// An Internet Archive title as a game title plus its region:
/// "XBOX Manual: 007 - Agent Under Fire (2002)(Electronic Arts)(US)" → ("007 - Agent Under Fire", Some("USA")).
pub fn clean_ia_title(raw: &str) -> (String, Option<&'static str>) {
    static GROUPS: OnceLock<Regex> = OnceLock::new();
    let groups = re(&GROUPS, r"\([^)]*\)|\[[^\]]*\]");
    let mut region = None;
    for g in groups.find_iter(raw) {
        for t in g.as_str().split(|c: char| !c.is_ascii_alphanumeric()).map(|t| t.to_ascii_lowercase()) {
            region = region.or(match t.as_str() {
                "usa" | "us" | "u" => Some("USA"),
                "eu" | "eur" | "europe" | "pal" | "e" | "uk" => Some("Europe"),
                "jp" | "jpn" | "japan" | "j" => Some("Japan"),
                _ => None,
            });
        }
    }
    let mut t = groups.replace_all(raw, " ").replace('_', " ");
    t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let trim = |s: &str| s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | ':' | ',' | '.' | '–')).to_string();
    loop {
        let before = t.clone();
        t = trim(&t);
        let lower = t.to_ascii_lowercase();
        for n in NOISE {
            if lower.starts_with(n) && lower[n.len()..].starts_with(|c: char| !c.is_ascii_alphanumeric()) {
                t = t[n.len()..].to_string();
                break;
            }
            if lower.ends_with(n) && lower[..lower.len() - n.len()].ends_with(|c: char| !c.is_ascii_alphanumeric()) {
                t = t[..t.len() - n.len()].to_string();
                break;
            }
        }
        if t == before {
            break;
        }
    }
    (trim(&t), region)
}

#[derive(Deserialize)]
struct Scrape {
    #[serde(default)]
    items: Vec<ScrapeItem>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct ScrapeItem {
    identifier: String,
    #[serde(default)]
    title: Option<serde_json::Value>,
}

fn text(v: &Option<serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a.first().and_then(|x| x.as_str()).unwrap_or_default().to_string(),
        _ => String::new(),
    }
}

fn valid_identifier(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Every item in an Internet Archive collection (the scrape API pages through big ones).
pub fn fetch_collection(fetch: &dyn Fetch, collection: &str) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let mut url = format!("https://archive.org/services/search/v1/scrape?q=collection%3A{collection}&fields=identifier%2Ctitle&count=10000");
        if let Some(c) = &cursor {
            url.push_str(&format!("&cursor={}", percent_encoding::utf8_percent_encode(c, percent_encoding::NON_ALPHANUMERIC)));
        }
        let body = fetch.get(&url)?.ok_or_else(|| Error::msg(format!("the Internet Archive has no collection {collection}")))?;
        let page: Scrape = serde_json::from_slice(&body)?;
        for it in page.items.into_iter().filter(|i| valid_identifier(&i.identifier)) {
            let raw = text(&it.title);
            let (title, region) = clean_ia_title(&raw);
            if title.is_empty() {
                continue;
            }
            out.push(Entry {
                name: match region {
                    Some(r) => format!("{title} ({r})"),
                    None => title,
                },
                url: format!("ia:{}", it.identifier),
                label: format!("Internet Archive · {}", if raw.is_empty() { &it.identifier } else { &raw }),
                size: None,
            });
        }
        match page.cursor {
            Some(c) if !c.is_empty() => cursor = Some(c),
            _ => break,
        }
    }
    Ok(out)
}

/// A list, from the cache when it's fresh; a stale copy when the Archive can't be reached.
fn cached_list(conn: &Connection, key: &str, load: impl FnOnce() -> Result<Vec<Entry>>) -> Result<Vec<Entry>> {
    let now = queries::now();
    let cached = queries::cache_get(conn, key)?;
    if let Some((body, at)) = &cached {
        if now - at < LIST_MAX_AGE_SECS {
            return Ok(serde_json::from_slice(body)?);
        }
    }
    match load() {
        Ok(list) => {
            queries::cache_put(conn, key, &serde_json::to_vec(&list)?, now)?;
            Ok(list)
        }
        Err(e) => match cached {
            Some((body, _)) => {
                log::warn!("couldn't refresh the manual list {key} ({e}); using the cached copy");
                Ok(serde_json::from_slice(&body)?)
            }
            None => Err(e),
        },
    }
}

/// A platform's lists, best source first. A list that can't be loaded is skipped (logged).
pub fn lists(conn: &Connection, fetch: &dyn Fetch, platform: &str) -> Vec<Vec<Entry>> {
    let mut out = Vec::new();
    if let Some(sys) = retrokit_system(platform) {
        let url = format!("https://archive.org/download/retrokit-manuals/{sys}/{sys}-compressed.zip/");
        match cached_list(conn, &format!("manuals:rk:{sys}"), || {
            let html = fetch.get(&url)?.ok_or_else(|| Error::msg("Retrokit's manual list is gone"))?;
            Ok(parse_retrokit(&String::from_utf8_lossy(&html)))
        }) {
            Ok(l) => out.push(l),
            Err(e) => log::warn!("Retrokit's {sys} manual list: {e}"),
        }
    }
    for c in ia_collections(platform) {
        match cached_list(conn, &format!("manuals:ia:{c}"), || fetch_collection(fetch, c)) {
            Ok(l) => out.push(l),
            Err(e) => log::warn!("Internet Archive manual list {c}: {e}"),
        }
    }
    out
}

/// What the matcher needs to know about a game.
pub struct GameKey {
    pub search_key: String,
    pub region: Option<String>,
}

/// A platform's lists with their title indexes, built once (matching a whole library reuses them).
pub struct Matcher {
    lists: Vec<(Vec<Entry>, Index)>,
}

impl Matcher {
    pub fn new(lists: Vec<Vec<Entry>>) -> Self {
        Self { lists: lists.into_iter().map(|l| { let i = Index::new(l.iter().map(|e| e.name.clone()).collect()); (l, i) }).collect() }
    }

    /// The game's manual: the first list (best source) with the same title, the game's region preferred.
    /// Only same-title matches count; anything looser is left to the picker.
    pub fn best(&self, game: &GameKey) -> Option<&Entry> {
        let q = matcher::Query { names: Vec::new(), search_key: &game.search_key, region: game.region.as_deref() };
        self.lists.iter().find_map(|(list, index)| {
            let m = index.best(&q).filter(|m| m.score >= 0.999)?;
            // The index hands back the name; find which entry it was.
            list.iter().find(|e| e.name == m.name)
        })
    }
}

#[cfg(test)]
pub fn best<'a>(lists: &'a [Vec<Entry>], game: &GameKey) -> Option<Entry> {
    Matcher::new(lists.to_vec()).best(game).cloned()
}

/// Picker options from the lists: same title first, then related titles, best source first within a score.
pub fn ranked(lists: &[Vec<Entry>], search_key: &str, region: Option<&str>, limit: usize) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for list in lists {
        let index = Index::new(list.iter().map(|e| e.name.clone()).collect());
        for m in index.ranked(search_key, region, limit) {
            if let Some(e) = list.iter().find(|e| e.name == m.name && !out.iter().any(|o| o.url == e.url)) {
                out.push(e.clone());
            }
        }
    }
    out.truncate(limit * 2);
    out
}

#[derive(Deserialize)]
struct Advanced {
    response: AdvancedResponse,
}

#[derive(Deserialize)]
struct AdvancedResponse {
    #[serde(default)]
    docs: Vec<AdvancedDoc>,
}

#[derive(Deserialize)]
struct AdvancedDoc {
    identifier: String,
    #[serde(default)]
    title: Option<serde_json::Value>,
    #[serde(default)]
    item_size: Option<u64>,
}

/// Internet Archive search for the picker: text items whose title has the words.
pub fn search(fetch: &dyn Fetch, words: &str) -> Result<Vec<Entry>> {
    let words: Vec<String> = words
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .take(8)
        .collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let q = format!("title:({}) AND mediatype:texts AND (manual OR instruction OR booklet)", words.join(" AND "));
    let enc = |s: &str| percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string();
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=item_size&rows=30&output=json",
        enc(&q)
    );
    let body = fetch.get(&url)?.ok_or_else(|| Error::msg("The Internet Archive's search isn't available right now."))?;
    let found: Advanced = serde_json::from_slice(&body)?;
    Ok(found
        .response
        .docs
        .into_iter()
        .filter(|d| valid_identifier(&d.identifier))
        .map(|d| {
            let title = text(&d.title);
            Entry {
                name: title.clone(),
                url: format!("ia:{}", d.identifier),
                label: format!("Internet Archive search · {}", if title.is_empty() { &d.identifier } else { &title }),
                size: d.item_size,
            }
        })
        .collect())
}

#[derive(Deserialize)]
struct Files {
    #[serde(default)]
    result: Vec<FileRow>,
}

#[derive(Deserialize)]
struct FileRow {
    name: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    size: Option<String>,
}

/// The file to download from an Internet Archive item: its scan PDF (the full one, not the
/// smaller "_text.pdf" copy), else the largest PDF, else a CBZ.
pub fn resolve_item(fetch: &dyn Fetch, identifier: &str) -> Result<String> {
    if !valid_identifier(identifier) {
        return Err(Error::msg("That isn't an Internet Archive item."));
    }
    let body = fetch
        .get(&format!("https://archive.org/metadata/{identifier}/files"))?
        .ok_or_else(|| Error::msg("That Internet Archive item is gone."))?;
    let files: Files = serde_json::from_slice(&body)?;
    let size = |f: &FileRow| f.size.as_deref().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let lower = |f: &FileRow| f.name.to_ascii_lowercase();
    let pdfs: Vec<&FileRow> = files.result.iter().filter(|f| lower(f).ends_with(".pdf") && !lower(f).ends_with("_text.pdf")).collect();
    let pick = pdfs
        .iter()
        .copied()
        .find(|f| f.format == "Image Container PDF")
        .or_else(|| pdfs.iter().copied().max_by_key(|f| size(f)))
        .or_else(|| files.result.iter().filter(|f| lower(f).ends_with(".cbz")).max_by_key(|f| size(f)))
        .ok_or_else(|| Error::msg("That Internet Archive item has no PDF or CBZ manual in it."))?;
    const PATH: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');
    let path: Vec<String> = pick.name.split('/').map(|p| percent_encoding::utf8_percent_encode(p, PATH).to_string()).collect();
    Ok(format!("https://archive.org/download/{identifier}/{}", path.join("/")))
}

/// Only Internet Archive addresses are ever downloaded from.
pub fn allowed_url(url: &str) -> bool {
    if let Some(id) = url.strip_prefix("ia:") {
        return valid_identifier(id);
    }
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split('/').next().unwrap_or("");
    host == "archive.org" || (host.ends_with(".us.archive.org") && host.starts_with("ia") && !host.contains('@'))
}
