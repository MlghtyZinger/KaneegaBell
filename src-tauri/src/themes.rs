//! Themes: a theme is data only (colors, corner radius, a focus look and an
//! icon), never CSS or script. KB checks every theme before using it (format,
//! value ranges, readable contrast, a visible focus ring) and turns its tokens
//! into the CSS variables the stylesheets already use. The built-in styles use
//! the same format, from `src-tauri/themes/`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

/// The theme format this build reads.
const FORMAT: u32 = 1;
pub const DEFAULT: &str = "ember";

/// Every color a theme sets, in the order of `tokens.css`.
pub const COLORS: &[&str] = &[
    "bg", "bg-raised", "bg-sunken", "surface", "surface-hover", "border", "text", "text-dim", "text-faint",
    "accent", "accent-strong", "on-accent", "focus", "gold", "danger", "ok",
];

/// How the focused cover is marked (other controls always get a ring).
pub const FOCUS_LOOKS: &[&str] = &["glow", "ring", "lift", "brackets", "bar", "spotlight"];

/// The bull icons, one per built-in style (`icons/themes/<id>.png`).
pub const ICONS: &[(&str, &[u8])] = &[
    ("ember", include_bytes!("../icons/themes/ember.png")),
    ("midnight", include_bytes!("../icons/themes/midnight.png")),
    ("daylight", include_bytes!("../icons/themes/daylight.png")),
    ("contrast", include_bytes!("../icons/themes/contrast.png")),
    ("moss", include_bytes!("../icons/themes/moss.png")),
    ("storm", include_bytes!("../icons/themes/storm.png")),
];

/// The built-in styles, in the picker's order (the default first).
const BUILT_IN: &[(&str, &str)] = &[
    ("ember", include_str!("../themes/ember.json")),
    ("midnight", include_str!("../themes/midnight.json")),
    ("daylight", include_str!("../themes/daylight.json")),
    ("contrast", include_str!("../themes/contrast.json")),
    ("moss", include_str!("../themes/moss.json")),
    ("storm", include_str!("../themes/storm.json")),
];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Theme {
    pub format: u32,
    pub name: String,
    pub author: String,
    pub version: String,
    pub min_app_version: String,
    /// "dark" or "light": native scrollbars and form controls follow it.
    pub scheme: String,
    pub radius: u32,
    pub focus_look: String,
    pub icon: String,
    pub colors: BTreeMap<String, String>,
    /// Colors that differ in TV Mode (any subset of `colors`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tv: BTreeMap<String, String>,
    /// Present when the theme has a picture (`background.webp` beside it); `dim` is how much of
    /// the style's background color is laid over it, in percent, to keep text readable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Background {
    pub dim: u32,
}

/// What the editor does with a theme's picture when saving.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "camelCase")]
pub enum PictureChange {
    Keep,
    Remove,
    /// A picture file the user chose (PNG, JPEG or WebP).
    Set(String),
}

/// A theme with its picture, as stored.
pub struct Loaded {
    pub theme: Theme,
    pub picture: Option<Vec<u8>>,
}

const MAX_JSON: u64 = 16 * 1024;
const MAX_PICTURE: u64 = 8 * 1024 * 1024;
const USER_PREFIX: &str = "u-";

/// What the page needs to apply a theme.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub id: String,
    pub css: String,
    pub focus_look: String,
    pub icon: String,
}

/// One entry of the style picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub focus_look: String,
    pub swatches: [String; 3],
    /// Made or imported by the user (can be edited and deleted).
    pub custom: bool,
}

impl Theme {
    /// Reads and checks a theme; also refused when it needs a KB newer than
    /// `app_version` (imports pass this build's version).
    fn parse_for(json: &str, app_version: &str) -> Result<Theme> {
        let theme: Theme = serde_json::from_str(json).map_err(|e| Error::msg(format!("not a KaneegaBell theme: {e}")))?;
        let mut problems = theme.problems();
        if newer(&theme.min_app_version, app_version) {
            problems.push(format!("it needs KaneegaBell {} or newer", theme.min_app_version));
        }
        if !problems.is_empty() {
            return Err(Error::msg(format!("{} can't be used: {}", theme.name, problems.join("; "))));
        }
        Ok(theme)
    }

    /// Everything wrong with the theme, in words for the user.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.format != FORMAT {
            out.push(format!("it's for theme format {}, this KB reads format {FORMAT}", self.format));
        }
        for (what, s) in [("name", &self.name), ("author", &self.author), ("version", &self.version)] {
            if s.trim().is_empty() || s.chars().count() > 40 || s.chars().any(char::is_control) {
                out.push(format!("its {what} must be 1 to 40 characters"));
            }
        }
        if !matches!(self.scheme.as_str(), "dark" | "light") {
            out.push("scheme must be \"dark\" or \"light\"".into());
        }
        if self.background.as_ref().is_some_and(|b| b.dim > 90) {
            out.push("background dim must be 0 to 90".into());
        }
        if self.radius > 24 {
            out.push("radius must be 0 to 24".into());
        }
        if !FOCUS_LOOKS.contains(&self.focus_look.as_str()) {
            out.push(format!("focusLook must be one of {}", FOCUS_LOOKS.join(", ")));
        }
        if !ICONS.iter().any(|(id, _)| *id == self.icon) {
            out.push(format!("icon must be one of {}", ICONS.iter().map(|(id, _)| *id).collect::<Vec<_>>().join(", ")));
        }
        for name in COLORS {
            match self.colors.get(*name) {
                None => out.push(format!("color \"{name}\" is missing")),
                Some(c) if rgb(c).is_none() => out.push(format!("color \"{name}\" must look like #1a2b3c")),
                _ => {}
            }
        }
        for (set, colors) in [("colors", &self.colors), ("tv", &self.tv)] {
            for (name, c) in colors {
                if !COLORS.contains(&name.as_str()) {
                    out.push(format!("{set} has an unknown color \"{name}\""));
                } else if set == "tv" && rgb(c).is_none() {
                    out.push(format!("tv color \"{name}\" must look like #1a2b3c"));
                }
            }
        }
        if out.is_empty() {
            self.check_contrast(&self.colors, "", &mut out);
            if !self.tv.is_empty() {
                let mut tv = self.colors.clone();
                tv.extend(self.tv.clone());
                self.check_contrast(&tv, "in TV Mode, ", &mut out);
            }
        }
        out
    }

    /// Text must stay readable and the focus ring visible, on a TV too.
    fn check_contrast(&self, c: &BTreeMap<String, String>, prefix: &str, out: &mut Vec<String>) {
        const PAIRS: &[(&str, &str, f64, &str)] = &[
            ("text", "bg", 4.5, "text"),
            ("text", "bg-raised", 4.5, "text on panels"),
            ("text", "surface", 4.5, "text on buttons"),
            ("text-dim", "bg", 4.5, "secondary text"),
            ("text-dim", "bg-raised", 4.5, "secondary text on panels"),
            ("text-faint", "bg", 3.0, "hints"),
            ("on-accent", "accent", 4.5, "text on the accent color"),
            ("focus", "bg", 3.0, "the focus ring"),
            ("focus", "bg-raised", 3.0, "the focus ring on panels"),
        ];
        for (fg, bg, min, what) in PAIRS {
            let r = contrast(&c[*fg], &c[*bg]);
            if r < *min {
                out.push(format!("{prefix}{what} is too faint ({r:.1}:1, needs {min}:1)"));
            }
        }
    }

    /// The CSS variables for this theme; `picture` (re-encoded WebP) becomes the window background.
    pub fn css(&self, picture: Option<&[u8]>) -> String {
        let vars = |c: &BTreeMap<String, String>| c.iter().map(|(k, v)| format!("--{k}:{v};")).collect::<String>();
        let mut css = format!(":root{{color-scheme:{};--radius:{}px;{}}}", self.scheme, self.radius, vars(&self.colors));
        if !self.tv.is_empty() {
            css.push_str(&format!(".tv{{{}}}", vars(&self.tv)));
        }
        if let (Some(bytes), Some(bg)) = (picture, &self.background) {
            use base64::Engine;
            let [r, g, b] = rgb(&self.colors["bg"]).unwrap_or([0, 0, 0]);
            let a = bg.dim as f64 / 100.0;
            css.push_str(&format!(
                "body{{background:linear-gradient(rgb({r} {g} {b}/{a}),rgb({r} {g} {b}/{a})),url(data:image/webp;base64,{}) center/cover no-repeat fixed var(--bg)}}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ));
        }
        css
    }

    fn summary(&self, id: &str) -> Summary {
        let custom = id.starts_with(USER_PREFIX);
        Summary {
            id: id.into(),
            name: self.name.clone(),
            focus_look: self.focus_look.clone(),
            swatches: ["bg", "accent", "focus"].map(|k| self.colors[k].clone()),
            custom,
        }
    }
}

/// `a` names a later version than `b` (both "x.y.z").
fn newer(a: &str, b: &str) -> bool {
    let parts = |s: &str| s.split('.').map(|p| p.trim().parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) > parts(b)
}

fn rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// WCAG contrast ratio of two #rrggbb colors.
fn contrast(a: &str, b: &str) -> f64 {
    let lum = |hex: &str| {
        let [r, g, b] = rgb(hex).unwrap_or([0, 0, 0]).map(|v| {
            let c = v as f64 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        });
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn built_in(id: &str) -> Option<Theme> {
    let (_, json) = BUILT_IN.iter().find(|(k, _)| *k == id)?;
    // Built in, so made for this version whatever it says.
    match Theme::parse_for(json, "999.0.0") {
        Ok(t) => Some(t),
        Err(e) => {
            log::error!("built-in theme {id}: {e}");
            None
        }
    }
}

fn user_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("themes")
}

/// Ids are `u-` plus lowercase letters, digits and dashes, so they can't name another folder.
fn valid_user_id(id: &str) -> bool {
    id.strip_prefix(USER_PREFIX)
        .is_some_and(|r| !r.is_empty() && r.len() <= 48 && r.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
}

/// A built-in style or one of the user's.
pub fn load(data_dir: &Path, id: &str) -> Option<Loaded> {
    if let Some(theme) = built_in(id) {
        return Some(Loaded { theme, picture: None });
    }
    if !valid_user_id(id) {
        return None;
    }
    let dir = user_dir(data_dir).join(id);
    let theme = Theme::parse_for(&std::fs::read_to_string(dir.join("theme.json")).ok()?, env!("CARGO_PKG_VERSION")).ok()?;
    let picture = theme.background.as_ref().and_then(|_| std::fs::read(dir.join("background.webp")).ok());
    Some(Loaded { theme, picture })
}

pub fn list(data_dir: &Path) -> Vec<Summary> {
    let mut out: Vec<Summary> = BUILT_IN.iter().filter_map(|(id, _)| built_in(id).map(|t| t.summary(id))).collect();
    let mut mine: Vec<Summary> = std::fs::read_dir(user_dir(data_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|id| load(data_dir, &id).map(|l| l.theme.summary(&id)))
        .collect();
    mine.sort_by_key(|s| s.name.to_lowercase());
    out.extend(mine);
    out
}

/// The theme saved in settings, or the default when it's unknown.
pub fn current(conn: &rusqlite::Connection, data_dir: &Path) -> (String, Loaded) {
    let id: Option<String> = crate::db::queries::get_setting(conn, "theme").ok().flatten();
    if let Some(id) = id {
        if let Some(t) = load(data_dir, &id) {
            return (id, t);
        }
        log::warn!("unknown theme {id}: using {DEFAULT}");
    }
    (DEFAULT.into(), load(data_dir, DEFAULT).expect("the default theme is valid"))
}

pub fn applied(id: &str, l: &Loaded) -> Applied {
    Applied { id: id.into(), css: l.theme.css(l.picture.as_deref()), focus_look: l.theme.focus_look.clone(), icon: l.theme.icon.clone() }
}

/// Runs before the page's own scripts, so the first paint already has the
/// theme: a constructed stylesheet over `tokens.css`, kept for later swaps.
pub fn init_script(applied: &Applied) -> String {
    let json = serde_json::to_string(applied).unwrap_or_else(|_| "null".into());
    format!(
        "(()=>{{const t={json};const s=new CSSStyleSheet();s.replaceSync(t.css);\
         document.adoptedStyleSheets=[...document.adoptedStyleSheets,s];window.__kbTheme={{sheet:s,...t}};}})();"
    )
}

/// The window and taskbar icon in the theme's colors.
pub fn set_window_icon(app: &AppHandle, theme: &Theme) {
    let Some((_, png)) = ICONS.iter().find(|(id, _)| *id == theme.icon) else { return };
    let Some(window) = app.get_webview_window("main") else { return };
    match image::load_from_memory_with_format(png, image::ImageFormat::Png) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (w, h) = rgba.dimensions();
            if let Err(e) = window.set_icon(tauri::image::Image::new_owned(rgba.into_raw(), w, h)) {
                log::warn!("couldn't set the window icon: {e}");
            }
        }
        Err(e) => log::warn!("theme icon {}: {e}", theme.icon),
    }
}

/// Saves the choice and returns what the page applies.
pub fn choose(app: &AppHandle, conn: &rusqlite::Connection, data_dir: &Path, id: &str) -> Result<Applied> {
    let l = load(data_dir, id).ok_or_else(|| Error::msg(format!("there's no style called {id}")))?;
    crate::db::queries::set_setting(conn, "theme", &serde_json::Value::String(id.into()))?;
    set_window_icon(app, &l.theme);
    Ok(applied(id, &l))
}

/// What the editor's live preview needs: the problems with an unsaved theme, and its CSS.
#[derive(Serialize)]
pub struct Checked {
    pub problems: Vec<String>,
    pub css: String,
}

/// The last picture the editor previewed, so changing a color doesn't re-encode it again.
static PREVIEW_PICTURE: std::sync::Mutex<Option<(String, Vec<u8>)>> = std::sync::Mutex::new(None);

/// `id` is the style being edited (for `Keep`).
pub fn check(data_dir: &Path, id: Option<&str>, theme: &Theme, picture: &PictureChange) -> Checked {
    let mut problems = theme.problems();
    let bytes = match picture {
        PictureChange::Keep => id.and_then(|i| load(data_dir, i)).and_then(|l| l.picture),
        PictureChange::Remove => None,
        PictureChange::Set(path) => {
            let mut cache = PREVIEW_PICTURE.lock().unwrap_or_else(|e| e.into_inner());
            match cache.as_ref().filter(|(p, _)| p == path) {
                Some((_, b)) => Some(b.clone()),
                None => match std::fs::metadata(path).map_err(Error::from).and_then(|m| if m.len() > MAX_PICTURE { Err(Error::msg("that picture is over 8 MB")) } else { Ok(()) })
                    .and_then(|_| reencode(&std::fs::read(path)?))
                {
                    Ok(b) => {
                        *cache = Some((path.clone(), b.clone()));
                        Some(b)
                    }
                    Err(e) => {
                        problems.push(e.to_string());
                        None
                    }
                },
            }
        }
    };
    let mut theme = theme.clone();
    if bytes.is_some() && theme.background.is_none() {
        theme.background = Some(Background { dim: 50 });
    }
    let css = if problems.is_empty() { theme.css(bytes.as_deref()) } else { String::new() };
    Checked { problems, css }
}

/// Resizes to at most 1920x1080 and re-encodes as WebP: whatever the file was, KB only ever stores
/// and serves its own encoding.
fn reencode(bytes: &[u8]) -> Result<Vec<u8>> {
    let img = crate::art::images::decode(bytes).map_err(|_| Error::msg("that picture can't be read (use PNG, JPEG or WebP)"))?;
    let img = if img.width() > 1920 || img.height() > 1080 { img.resize(1920, 1080, image::imageops::FilterType::Triangle) } else { img };
    crate::art::images::webp(&img, 78.0)
}

fn slug(name: &str) -> String {
    let s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let s: String = s.chars().take(40).collect();
    if s.is_empty() { "theme".into() } else { s }
}

/// Saves a theme of the user's: a new one (`id` None) or an edit. Returns its id.
pub fn save(data_dir: &Path, id: Option<&str>, mut theme: Theme, picture: &PictureChange) -> Result<String> {
    theme.format = FORMAT;
    theme.min_app_version = env!("CARGO_PKG_VERSION").into();
    let root = user_dir(data_dir);
    let id = match id {
        Some(id) if valid_user_id(id) => id.to_string(),
        Some(_) => return Err(Error::msg("only your own themes can be changed")),
        None => (1..)
            .map(|n| if n == 1 { format!("{USER_PREFIX}{}", slug(&theme.name)) } else { format!("{USER_PREFIX}{}-{n}", slug(&theme.name)) })
            .find(|c| !root.join(c).exists())
            .expect("an unused id"),
    };
    let dir = root.join(&id);
    let new = match picture {
        PictureChange::Keep => std::fs::read(dir.join("background.webp")).ok(),
        PictureChange::Remove => None,
        PictureChange::Set(path) => {
            if std::fs::metadata(path)?.len() > MAX_PICTURE {
                return Err(Error::msg("that picture is over 8 MB"));
            }
            Some(reencode(&std::fs::read(path)?)?)
        }
    };
    theme.background = match (&new, theme.background.take()) {
        (Some(_), Some(b)) => Some(b),
        (Some(_), None) => Some(Background { dim: 50 }),
        (None, _) => None,
    };
    let problems = theme.problems();
    if !problems.is_empty() {
        return Err(Error::msg(problems.join("; ")));
    }
    std::fs::create_dir_all(&dir)?;
    match &new {
        Some(bytes) => std::fs::write(dir.join("background.webp"), bytes)?,
        None => {
            let _ = std::fs::remove_file(dir.join("background.webp"));
        }
    }
    std::fs::write(dir.join("theme.json"), serde_json::to_string_pretty(&theme).map_err(|e| Error::msg(e.to_string()))?)?;
    Ok(id)
}

pub fn delete(data_dir: &Path, id: &str) -> Result<()> {
    if !valid_user_id(id) {
        return Err(Error::msg("built-in styles can't be deleted"));
    }
    let dir = user_dir(data_dir).join(id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Writes `<folder>/<name>.kbtheme` (theme.json plus the picture, if any); returns the path.
pub fn export(data_dir: &Path, id: &str, folder: &Path) -> Result<PathBuf> {
    let l = load(data_dir, id).ok_or_else(|| Error::msg("that style doesn't exist"))?;
    let mut theme = l.theme;
    if l.picture.is_none() {
        theme.background = None;
    }
    let path = folder.join(format!("{}.kbtheme", slug(&theme.name)));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path)?);
    let opts = zip::write::SimpleFileOptions::default();
    let zerr = |e: zip::result::ZipError| Error::msg(e.to_string());
    zip.start_file("theme.json", opts).map_err(zerr)?;
    zip.write_all(serde_json::to_string_pretty(&theme).map_err(|e| Error::msg(e.to_string()))?.as_bytes())?;
    if let Some(bytes) = &l.picture {
        zip.start_file("background.webp", opts).map_err(zerr)?;
        zip.write_all(bytes)?;
    }
    zip.finish().map_err(zerr)?;
    Ok(path)
}

/// Reads a `.kbtheme` and saves it as one of the user's themes. Nothing in the zip is ever
/// extracted by name: only `theme.json` and one `background.*` picture are looked up, anything
/// else makes the file invalid, and the picture is re-encoded.
pub fn import(data_dir: &Path, file: &Path) -> Result<String> {
    if std::fs::metadata(file)?.len() > MAX_PICTURE + MAX_JSON {
        return Err(Error::msg("that file is too big to be a theme"));
    }
    let bad = |e: zip::result::ZipError| Error::msg(format!("not a KaneegaBell theme: {e}"));
    let mut zip = zip::ZipArchive::new(Cursor::new(std::fs::read(file)?)).map_err(bad)?;
    if zip.len() > 2 {
        return Err(Error::msg("a theme file holds only theme.json and one background picture"));
    }
    let (mut json, mut picture) = (None, None);
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(bad)?;
        let name = entry.name().to_string();
        let (limit, slot) = match name.as_str() {
            "theme.json" => (MAX_JSON, &mut json),
            "background.webp" | "background.png" | "background.jpg" | "background.jpeg" => (MAX_PICTURE, &mut picture),
            _ => return Err(Error::msg(format!("a theme file may not contain \"{name}\""))),
        };
        let mut buf = Vec::new();
        entry.take(limit + 1).read_to_end(&mut buf)?;
        if buf.len() as u64 > limit {
            return Err(Error::msg(format!("{name} is too big")));
        }
        *slot = Some(buf);
    }
    let json = json.ok_or_else(|| Error::msg("theme.json is missing"))?;
    let theme = Theme::parse_for(&String::from_utf8(json).map_err(|_| Error::msg("theme.json isn't text"))?, env!("CARGO_PKG_VERSION"))?;
    let Some(bytes) = picture else { return save(data_dir, None, theme, &PictureChange::Remove) };
    // `save` takes a file, so the re-encoded picture goes through a temporary one.
    let tmp = user_dir(data_dir).join("import.tmp");
    std::fs::create_dir_all(user_dir(data_dir))?;
    std::fs::write(&tmp, reencode(&bytes)?)?;
    let id = save(data_dir, None, theme, &PictureChange::Set(tmp.to_string_lossy().into_owned()));
    let _ = std::fs::remove_file(&tmp);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_are_valid_and_readable() {
        for (id, json) in BUILT_IN {
            let t = Theme::parse_for(json, "0.3.11").unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(t.icon, *id, "{id} uses its own icon");
        }
        let none = tempfile::tempdir().unwrap();
        assert_eq!(list(none.path()).len(), BUILT_IN.len());
        assert_eq!(list(none.path())[0].id, DEFAULT);
    }

    fn ember() -> Theme {
        built_in(DEFAULT).unwrap()
    }

    #[test]
    fn rejects_bad_values() {
        let mut t = ember();
        t.colors.insert("bg".into(), "red".into());
        t.colors.remove("focus");
        t.colors.insert("background-image".into(), "#000000".into());
        t.focus_look = "sparkle".into();
        t.radius = 400;
        t.name = "x".repeat(41);
        let p = t.problems().join("\n");
        for needle in ["\"bg\" must look like", "\"focus\" is missing", "unknown color \"background-image\"", "focusLook", "radius", "name"] {
            assert!(p.contains(needle), "missing {needle:?} in {p}");
        }
    }

    #[test]
    fn rejects_css_smuggled_in_a_color() {
        let mut t = ember();
        t.colors.insert("accent".into(), "#fff;}body{display:none".into());
        assert!(t.problems().iter().any(|p| p.contains("\"accent\"")));
        assert!(Theme::parse_for(&serde_json::to_string(&t).unwrap(), "0.3.11").is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_newer_formats() {
        let json = BUILT_IN[0].1.replacen("\"format\": 1", "\"format\": 1, \"script\": \"alert(1)\"", 1);
        assert!(Theme::parse_for(&json, "0.3.11").is_err());
        let mut t = ember();
        t.format = 2;
        t.min_app_version = "9.0.0".into();
        let e = Theme::parse_for(&serde_json::to_string(&t).unwrap(), "0.3.11").unwrap_err().to_string();
        assert!(e.contains("format 2") && e.contains("9.0.0"), "{e}");
    }

    #[test]
    fn invisible_focus_is_refused() {
        let mut t = ember();
        let bg = t.colors["bg"].clone();
        t.colors.insert("focus".into(), bg);
        assert!(t.problems().iter().any(|p| p.contains("focus ring")));
        let mut tv = ember();
        tv.tv.insert("text".into(), "#1a1512".into());
        assert!(tv.problems().iter().any(|p| p.starts_with("in TV Mode, text")));
    }

    #[test]
    fn css_has_every_token() {
        let css = ember().css(None);
        assert!(css.starts_with(":root{color-scheme:dark;--radius:6px;"));
        for c in COLORS {
            assert!(css.contains(&format!("--{c}:#")), "{c}");
        }
        assert!(!css.contains(".tv{"));
    }

    #[test]
    fn contrast_matches_wcag() {
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 0.01);
        assert!((contrast("#777777", "#777777") - 1.0).abs() < 0.001);
        assert!(newer("0.4.0", "0.3.11") && !newer("0.3.11", "0.3.11") && !newer("0.3.2", "0.3.11"));
    }

    fn png(path: &Path) {
        image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 30, 30, 255])).save(path).unwrap();
    }

    #[test]
    fn save_load_export_import_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pic = dir.path().join("p.png");
        png(&pic);
        let mut t = ember();
        t.name = "My Red!".into();
        let id = save(dir.path(), None, t.clone(), &PictureChange::Set(pic.to_string_lossy().into())).unwrap();
        assert_eq!(id, "u-my-red");
        // The same name again gets its own id.
        assert_eq!(save(dir.path(), None, t.clone(), &PictureChange::Remove).unwrap(), "u-my-red-2");
        let l = load(dir.path(), &id).unwrap();
        assert!(l.picture.is_some() && l.theme.background.is_some());
        assert!(l.theme.css(l.picture.as_deref()).contains("url(data:image/webp;base64,"));
        assert!(list(dir.path()).iter().any(|s| s.id == id && s.custom));

        let out = tempfile::tempdir().unwrap();
        let file = export(dir.path(), &id, out.path()).unwrap();
        let back = import(dir.path(), &file).unwrap();
        assert_eq!(back, "u-my-red-3");
        assert!(load(dir.path(), &back).unwrap().picture.is_some());

        delete(dir.path(), &id).unwrap();
        assert!(load(dir.path(), &id).is_none());
        assert!(delete(dir.path(), "ember").is_err() && delete(dir.path(), "u-../x").is_err());
    }

    #[test]
    fn save_refuses_unreadable_themes() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = ember();
        t.colors.insert("focus".into(), t.colors["bg"].clone());
        assert!(save(dir.path(), None, t, &PictureChange::Remove).unwrap_err().to_string().contains("focus ring"));
        assert!(save(dir.path(), Some("ember"), ember(), &PictureChange::Remove).is_err());
        assert!(!user_dir(dir.path()).exists());
    }

    fn zip_of(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, b) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn import_refuses_hostile_files() {
        let dir = tempfile::tempdir().unwrap();
        let json = serde_json::to_vec(&ember()).unwrap();
        let f = dir.path().join("x.kbtheme");
        for (files, needle) in [
            (vec![("../evil.json", &json[..]), ("theme.json", &json[..])], "may not contain"),
            (vec![("theme.json", &json[..]), ("run.exe", b"MZ")], "may not contain"),
            (vec![("a", &b""[..]), ("b", &b""[..]), ("theme.json", &json[..])], "only theme.json"),
            (vec![("background.png", &b"x"[..])], "theme.json is missing"),
            (vec![("theme.json", &b"{\"format\":1}"[..])], "not a KaneegaBell theme"),
            (vec![("theme.json", &vec![b' '; 20_000][..])], "too big"),
        ] {
            zip_of(&f, &files);
            let e = import(dir.path(), &f).unwrap_err().to_string();
            assert!(e.contains(needle), "{needle}: {e}");
        }
        std::fs::write(&f, b"not a zip").unwrap();
        assert!(import(dir.path(), &f).is_err());
        // A picture that isn't one.
        zip_of(&f, &[("theme.json", &json[..]), ("background.png", b"not an image")]);
        assert!(import(dir.path(), &f).unwrap_err().to_string().contains("can't be read"));
        assert!(list(dir.path()).iter().all(|s| !s.custom));
    }
}
