//! Themes: a theme is data only (colors, corner radius, a focus look and an
//! icon), never CSS or script. KB checks every theme before using it (format,
//! value ranges, readable contrast, a visible focus ring) and turns its tokens
//! into the CSS variables the stylesheets already use. The built-in styles use
//! the same format, from `src-tauri/themes/`.

use std::collections::BTreeMap;

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
}

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

    /// The CSS variables for this theme.
    pub fn css(&self) -> String {
        let vars = |c: &BTreeMap<String, String>| c.iter().map(|(k, v)| format!("--{k}:{v};")).collect::<String>();
        let mut css = format!(":root{{color-scheme:{};--radius:{}px;{}}}", self.scheme, self.radius, vars(&self.colors));
        if !self.tv.is_empty() {
            css.push_str(&format!(".tv{{{}}}", vars(&self.tv)));
        }
        css
    }

    fn summary(&self, id: &str) -> Summary {
        Summary {
            id: id.into(),
            name: self.name.clone(),
            focus_look: self.focus_look.clone(),
            swatches: ["bg", "accent", "focus"].map(|k| self.colors[k].clone()),
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

pub fn list() -> Vec<Summary> {
    BUILT_IN.iter().filter_map(|(id, _)| built_in(id).map(|t| t.summary(id))).collect()
}

/// The theme saved in settings, or the default when it's unknown.
pub fn current(conn: &rusqlite::Connection) -> (String, Theme) {
    let id: Option<String> = crate::db::queries::get_setting(conn, "theme").ok().flatten();
    if let Some(id) = id {
        if let Some(t) = built_in(&id) {
            return (id, t);
        }
        log::warn!("unknown theme {id}: using {DEFAULT}");
    }
    (DEFAULT.into(), built_in(DEFAULT).expect("the default theme is valid"))
}

pub fn applied(id: &str, theme: &Theme) -> Applied {
    Applied { id: id.into(), css: theme.css(), focus_look: theme.focus_look.clone(), icon: theme.icon.clone() }
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
pub fn choose(app: &AppHandle, conn: &rusqlite::Connection, id: &str) -> Result<Applied> {
    let theme = built_in(id).ok_or_else(|| Error::msg(format!("there's no style called {id}")))?;
    crate::db::queries::set_setting(conn, "theme", &serde_json::Value::String(id.into()))?;
    set_window_icon(app, &theme);
    Ok(applied(id, &theme))
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
        assert_eq!(list().len(), BUILT_IN.len());
        assert_eq!(list()[0].id, DEFAULT);
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
        let css = ember().css();
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
}
