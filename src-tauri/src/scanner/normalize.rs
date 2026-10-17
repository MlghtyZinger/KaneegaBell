//! Filename → title normalization.
//!
//! Handles No-Intro (`Title (USA) (En,Fr)`), GoodTools (`Title (U) [!]`), scene
//! releases (`Title_Words_USA_PROPER_NDS-Group`), release-number prefixes
//! (`3055 - Title`), 3DS title-ID prefixes, `Title, The` article inversion and
//! hack/translation tags. Display titles keep edition/hack info as a separate
//! `variant`; the `search_key` strips everything for metadata matching.

use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    /// Display title, e.g. "The Legend of Zelda: The Wind Waker".
    pub title: String,
    /// Kept tags that distinguish this release: hacks, translations, editions.
    pub variant: Option<String>,
    pub region: Option<String>,
    pub disc: Option<u32>,
    /// Game ID found in the name, e.g. "RFEE01" or "CTR-P-AMGE".
    pub serial: Option<String>,
    /// Lowercase, leading article removed, digit runs zero-padded for natural order.
    pub sort_title: String,
    /// Lowercase ASCII words only; used for metadata search.
    pub search_key: String,
}

impl ParsedName {
    /// Key used to group the discs of one game: same title and same variant.
    pub fn group_key(&self) -> String {
        match &self.variant {
            Some(v) => format!("{}|{}", self.search_key, search_key(v)),
            None => self.search_key.clone(),
        }
    }
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

re!(SUFFIX_JUNK, r"(?i)\.(piratelegit|decrypted|trimmed|trim)$");
re!(TITLE_ID_PREFIX, r"^[0-9A-Fa-f]{16}\s+");
re!(RELEASE_NO_PREFIX, r"^\d{4}\s+-\s+");
re!(SCENE_GROUP, r"-[A-Za-z0-9]+$");
re!(TAG, r"\s*[\(\[]([^\)\]]*)[\)\]]");
re!(DISC, r"(?i)^(?:disc|disk|cd)\s*(\d+)(?:\s*of\s*\d+)?$");
re!(TRACK, r"(?i)^track\s*\d+$");
re!(VERSION, r"(?i)^(?:rev\s*[0-9a-z.]+|v\s*\d+(?:\.\d+)*[a-z]?)$");
re!(MULTI_LANG, r"^M\d+$");
re!(GOODTOOLS_FLAG, r"^(?:!|a\d*|b\d*|o\d*|f\d*|p\d*)$");
re!(SERIAL_DOLPHIN, r"^[A-Z][A-Z0-9]{2}[A-Z][A-Z0-9]{2}$");
re!(SERIAL_3DS, r"^CTR-[A-Z]-[A-Z0-9]{4}$");
re!(SERIAL_PS, r"^S[CL][EKPU][MS][-_ ]?\d{3}\.?\d{2}$");
re!(SERIAL_SWITCH, r"^01[0-9A-Fa-f]{14}$");
// "Super Mario Galaxy v1.0.0": a version outside brackets, dropped like a bracketed one.
re!(TRAILING_VERSION, r"(?i)\s+v\d+(?:\.\d+)+$");
re!(ANY_VERSION, r"(?i)\bv\d+(?:\.\d+)+");
re!(ARTICLE_SUFFIX, r"^(.+?), (The|A|An)((?:\s+-\s+|:\s*).*)?$");
re!(LEADING_ARTICLE, r"^(?:the|a|an)\s+");
re!(DIGITS, r"\d+");
re!(SPACES, r"\s+");

const REGION_WORDS: &[&str] = &[
    "usa", "europe", "japan", "world", "australia", "korea", "china", "asia", "brazil",
    "canada", "france", "germany", "spain", "italy", "netherlands", "sweden", "uk",
    "taiwan", "hong kong", "russia", "scandinavia", "us", "eu", "pal", "ntsc", "ntsc-u", "ntsc-j",
];

/// GoodTools single/double-letter region codes.
const REGION_CODES: &[(&str, &str)] = &[
    ("U", "USA"),
    ("US", "USA"),
    ("E", "Europe"),
    ("J", "Japan"),
    ("W", "World"),
    ("UE", "USA, Europe"),
    ("JU", "Japan, USA"),
    ("JUE", "World"),
    ("EU", "Europe"),
    ("A", "Australia"),
    ("K", "Korea"),
    ("F", "France"),
    ("G", "Germany"),
    ("S", "Spain"),
    ("I", "Italy"),
    ("C", "China"),
];

const LANG_CODES: &[&str] = &[
    "en", "fr", "de", "es", "it", "nl", "pt", "sv", "no", "da", "fi", "ja", "ko", "zh", "ru",
    "pl", "ca", "el", "hu", "cs", "tr", "ar",
];

/// Dumper/release-group names that appear as `(U)(GroupName)` in NDS/GBA sets.
const DUMP_GROUPS: &[&str] = &[
    "xenophobia", "trashman", "venom", "independent", "eximius", "legacy", "squire", "wrg",
    "mode7", "eurasia", "rising sun", "lms", "supremacy", "psyfer", "caravan", "micronauts",
    "nukethis", "ind", "lady martini", "pucrunch", "evasion", "suxxors", "megasoft",
    "goodnight", "cluster", "dsrp", "hadouken", "sir vg", "blackbag", "puppa", "sun",
];

/// Scene-release tokens stripped from the end of `Words_Like_This` names.
const SCENE_JUNK: &[&str] = &[
    "USA", "US", "EUR", "EU", "JPN", "JAP", "JP", "PAL", "NTSC", "PROPER", "REPACK", "INTERNAL",
    "READNFO", "NFOFIX", "DUMPFIX", "DIRFIX", "MULTI", "MULTI2", "MULTI3", "MULTI4", "MULTI5",
    "MULTI6", "MULTi2", "MULTi3", "MULTi4", "MULTi5", "MULTi6", "NDS", "DS", "PSP", "GBA",
    "N64", "PS1", "PSX", "PS2", "PS3", "GC", "NGC", "WII", "3DS", "SNES", "iNTERNAL",
];

fn scene_region(token: &str) -> Option<&'static str> {
    match token {
        "USA" | "US" => Some("USA"),
        "EUR" | "EU" | "PAL" => Some("Europe"),
        "JPN" | "JAP" | "JP" => Some("Japan"),
        _ => None,
    }
}

enum Tag {
    Disc(u32),
    Region(String),
    Serial(String),
    Drop,
    Keep(String),
}

fn classify(tag: &str) -> Tag {
    let t = tag.trim();
    if t.is_empty() {
        return Tag::Drop;
    }
    if let Some(c) = DISC.captures(t) {
        return Tag::Disc(c[1].parse().unwrap_or(1));
    }
    // "BASE" / "APP": Switch release tags for the game itself.
    if TRACK.is_match(t) || VERSION.is_match(t) || MULTI_LANG.is_match(t) || GOODTOOLS_FLAG.is_match(t) || t.eq_ignore_ascii_case("base") || t.eq_ignore_ascii_case("app") {
        return Tag::Drop;
    }
    if SERIAL_SWITCH.is_match(t) {
        return Tag::Serial(t.to_ascii_uppercase());
    }
    if SERIAL_3DS.is_match(t) || SERIAL_DOLPHIN.is_match(t) || SERIAL_PS.is_match(t) {
        // A 6-char all-caps word like "UNDUB" or "HACKED" isn't a serial: Dolphin
        // IDs always end in a two-character maker code containing a digit or are mixed.
        if !SERIAL_DOLPHIN.is_match(t) || t.chars().any(|c| c.is_ascii_digit()) {
            return Tag::Serial(t.to_string());
        }
    }
    if let Some((_, name)) = REGION_CODES.iter().find(|(code, _)| *code == t) {
        return Tag::Region((*name).to_string());
    }
    let parts: Vec<String> = t.split([',', '+']).map(|p| p.trim().to_ascii_lowercase()).collect();
    if parts.iter().all(|p| REGION_WORDS.contains(&p.as_str())) {
        return Tag::Region(t.to_string());
    }
    if parts.iter().all(|p| LANG_CODES.contains(&p.as_str())) {
        return Tag::Drop;
    }
    if DUMP_GROUPS.contains(&t.to_ascii_lowercase().as_str()) {
        return Tag::Drop;
    }
    Tag::Keep(t.to_string())
}

/// Folds common accented Latin characters to ASCII.
fn fold_char(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        c => c,
    }
}

/// Lowercase ASCII alphanumeric words separated by single spaces.
pub fn search_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.chars().map(fold_char) {
        match c {
            '\'' | '’' | '`' => {} // "Luigi's" -> "luigis"
            '&' => {
                out.push_str(if out.is_empty() { "and" } else { " and" });
                pending_space = true;
            }
            c if c.is_alphanumeric() => {
                if pending_space && !out.is_empty() {
                    out.push(' ');
                }
                pending_space = false;
                out.extend(c.to_lowercase());
            }
            _ => pending_space = true,
        }
    }
    out
}

fn sort_title(title: &str) -> String {
    let key = search_key(title);
    let key = LEADING_ARTICLE.replace(&key, "");
    DIGITS
        .replace_all(&key, |c: &regex::Captures| format!("{:0>5}", &c[0]))
        .into_owned()
}

fn is_scene_style(s: &str) -> bool {
    !s.contains(' ') && s.contains('_')
}

/// Turns `Golden_Sun_Dark_Dawn_USA_PROPER_NDS-NukeThis` into words, returning
/// the cleaned title and any region found among the stripped tokens.
fn clean_scene(s: &str) -> (String, Option<String>) {
    let s = SCENE_GROUP.replace(s, "");
    let mut tokens: Vec<&str> = s.split('_').filter(|t| !t.is_empty()).collect();
    let mut region = None;
    while tokens.len() > 1 {
        let last = *tokens.last().unwrap();
        if !SCENE_JUNK.contains(&last) {
            break;
        }
        if region.is_none() {
            region = scene_region(last).map(str::to_string);
        }
        tokens.pop();
    }
    (tokens.join(" "), region)
}

pub fn parse(stem: &str) -> ParsedName {
    let mut s = stem.trim().to_string();
    s = SUFFIX_JUNK.replace(&s, "").into_owned();
    s = TITLE_ID_PREFIX.replace(&s, "").into_owned();
    s = RELEASE_NO_PREFIX.replace(&s, "").into_owned();

    let mut region = None;
    if is_scene_style(&s) {
        let (clean, r) = clean_scene(&s);
        s = clean;
        region = r;
    } else {
        // "Persona Q_ Shadow of the Labyrinth": an underscore stands in for a colon.
        s = s.replace("_ ", ": ").replace('_', " ");
    }

    let mut disc = None;
    let mut serial = None;
    let mut kept: Vec<String> = Vec::new();
    for cap in TAG.captures_iter(&s) {
        match classify(&cap[1]) {
            Tag::Disc(n) => disc = Some(n),
            Tag::Region(r) => {
                if region.is_none() {
                    region = Some(r)
                }
            }
            Tag::Serial(id) => serial = Some(id),
            Tag::Drop => {}
            Tag::Keep(k) => kept.push(k),
        }
    }
    let mut title = TAG.replace_all(&s, "").into_owned();
    title = SPACES.replace_all(title.trim(), " ").into_owned();
    title = title.trim_end_matches([' ', '-', ':', ',']).to_string();
    // Only a lone version: "ENG Fix v2.1.5 + Recolor v2.0" names a hack's parts.
    if ANY_VERSION.find_iter(&title).count() == 1 {
        title = TRAILING_VERSION.replace(&title, "").into_owned();
    }

    if let Some(c) = ARTICLE_SUFFIX.captures(&title) {
        let rest = c.get(3).map_or("", |m| m.as_str());
        title = format!("{} {}{}", &c[2], &c[1], rest);
    }
    // No-Intro uses " - " where the real title has a colon.
    if let Some(i) = title.find(" - ") {
        title.replace_range(i..i + 3, ": ");
    }
    if title.is_empty() {
        title = stem.trim().to_string();
    }

    let variant = (!kept.is_empty()).then(|| kept.join(" · "));
    ParsedName {
        sort_title: sort_title(&title),
        search_key: search_key(&title),
        title,
        variant,
        region,
        disc,
        serial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(stem: &str) -> ParsedName {
        parse(stem)
    }

    #[track_caller]
    fn title(stem: &str, expected: &str) {
        assert_eq!(parse(stem).title, expected, "input: {stem}");
    }

    #[test]
    fn no_intro_names() {
        title("Fire Emblem Echoes - Shadows of Valentia (USA) (En,Fr,Es)", "Fire Emblem Echoes: Shadows of Valentia");
        title("Kirby Super Star (USA)", "Kirby Super Star");
        title("Golden Sun - The Lost Age (USA, Europe)", "Golden Sun: The Lost Age");
        title("Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)", "Jak II");
        title("Star Wars - Episode III - Revenge of the Sith (USA)", "Star Wars: Episode III - Revenge of the Sith");
        let p = t("Fire Emblem - The Blazing Blade (USA, Australia)");
        assert_eq!(p.region.as_deref(), Some("USA, Australia"));
        assert_eq!(p.variant, None);
    }

    #[test]
    fn article_inversion() {
        title("Legend of Zelda, The - The Wind Waker (USA)", "The Legend of Zelda: The Wind Waker");
        title("Darkness, The (USA) (En,Fr,De,Es,It)", "The Darkness");
        title("Godfather, The - The Don's Edition (USA)", "The Godfather: The Don's Edition");
        assert_eq!(t("Darkness, The (USA)").sort_title, "darkness");
    }

    #[test]
    fn goodtools_and_release_numbers() {
        let p = t("3055 - Chrono Trigger (U)(XenoPhobia)");
        assert_eq!(p.title, "Chrono Trigger");
        assert_eq!(p.region.as_deref(), Some("USA"));
        assert_eq!(p.variant, None);
        title("3424 - Dragon Quest V - Hand of the Heavenly Bride (US)(M3)(XenoPhobia)", "Dragon Quest V: Hand of the Heavenly Bride");
        title("4185 - Kirby Super Star Ultra (Europe) (En,Fr,De,Es,It)", "Kirby Super Star Ultra");
        title("Metroid - Zero Mission (U)(TrashMan)", "Metroid: Zero Mission");
        title("Fire Emblem - The Sacred Stones (U)", "Fire Emblem: The Sacred Stones");
        title("Super Mario World (U) [!]", "Super Mario World");
    }

    #[test]
    fn scene_releases() {
        let p = t("Golden_Sun_Dark_Dawn_USA_PROPER_NDS-NukeThis");
        assert_eq!(p.title, "Golden Sun Dark Dawn");
        assert_eq!(p.region.as_deref(), Some("USA"));
        title("Fire_Emblem_Shadow_Dragon_REPACK_USA_NDS-Micronauts", "Fire Emblem Shadow Dragon");
        title("Castlevania_Dracula_X_Chronicles_USA_INTERNAL_READNFO_PSP-pSyPSP", "Castlevania Dracula X Chronicles");
        title("Fairy_Tail_Portable_Guild_2_JPN_PSP-Caravan", "Fairy Tail Portable Guild 2");
        title("Initital_D_Street_Stage_JAP_PSP-DMU", "Initital D Street Stage");
        title("Fairy_Tail_Zelef_Kakusei_JPN_PSP-PLAYASiA", "Fairy Tail Zelef Kakusei");
    }

    #[test]
    fn three_ds_prefixes_serials_and_suffixes() {
        let p = t("0004000000081E00 Metal Gear Solid 3D - Snake Eater (CTR-P-AMGE) (v0.0.0) (W).piratelegit");
        assert_eq!(p.title, "Metal Gear Solid 3D: Snake Eater");
        assert_eq!(p.serial.as_deref(), Some("CTR-P-AMGE"));
        assert_eq!(p.region.as_deref(), Some("World"));
        assert_eq!(p.variant, None);
        title("00040000001D7100 Persona Q2 - New Cinema Labyrinth (CTR-P-AQ2E) (v0.1.0) (U).piratelegit", "Persona Q2: New Cinema Labyrinth");
        title("Persona Q_ Shadow of the Labyrinth", "Persona Q: Shadow of the Labyrinth");
    }

    #[test]
    fn dolphin_game_ids() {
        let p = t("Fire Emblem - Radiant Dawn [RFEE01]");
        assert_eq!(p.title, "Fire Emblem: Radiant Dawn");
        assert_eq!(p.serial.as_deref(), Some("RFEE01"));
        assert_eq!(t("Super Mario Galaxy 2 [SB4E01]").serial.as_deref(), Some("SB4E01"));
        // All-caps words are not serials.
        assert_eq!(t("Final Fantasy X International (Japan) [UNDUBS]").serial, None);
    }

    #[test]
    fn switch_title_ids_and_tags() {
        let p = t("The Legend of Zelda - Tears of the Kingdom [0100f2c0115b6000][v0] [BASE]");
        assert_eq!(p.title, "The Legend of Zelda: Tears of the Kingdom");
        assert_eq!(p.serial.as_deref(), Some("0100F2C0115B6000"));
        assert_eq!(p.variant, None, "[v0] and [BASE] aren't variants");
        assert_eq!(t("Super Mario Odyssey (USA) [01000000000100000]").serial, None, "17 digits is no title ID");
        // The owner's eShop dump: no version and no "APP" in the title.
        let p = t("Super Mario Galaxy 2 v1.0.0[0100FD8022DAA000][v0][APP]");
        assert_eq!((p.title.as_str(), p.variant.as_deref()), ("Super Mario Galaxy 2", None));
        title("Pokemon Radical Red v4.1", "Pokemon Radical Red");
    }

    #[test]
    fn hacks_and_translations_are_kept_as_variant() {
        let p = t("Mother 3 (Japan) [T-En by Chewy & Jeffman & Tomato v1.3a] [HQ audio mix by MusicTheorist]");
        assert_eq!(p.title, "Mother 3");
        assert_eq!(p.region.as_deref(), Some("Japan"));
        assert_eq!(
            p.variant.as_deref(),
            Some("T-En by Chewy & Jeffman & Tomato v1.3a · HQ audio mix by MusicTheorist")
        );
        let p = t("Castlevania - Dawn of Dissonance (v1.0.5) (Belnades Blood Edition) (USA)");
        assert_eq!(p.title, "Castlevania: Dawn of Dissonance");
        assert_eq!(p.variant.as_deref(), Some("Belnades Blood Edition"));
        assert_eq!(t("Fire Emblem - The Binding Blade (English v1.2.1)").variant.as_deref(), Some("English v1.2.1"));
        assert_eq!(t("Final Fantasy X International (Japan) [Undub]").variant.as_deref(), Some("Undub"));
        assert_eq!(t("Persona 2 Innocent Sin (USA) (Undub v1.0)").variant.as_deref(), Some("Undub v1.0"));
        assert_eq!(t("Brightis (Japan) [En-v1.0.1]").variant.as_deref(), Some("En-v1.0.1"));
        title("Mother 2 - Perfect Edition (v1.53)", "Mother 2: Perfect Edition");
    }

    #[test]
    fn disc_and_track_tags() {
        let p = t("Metal Gear Solid - The Twin Snakes (Disc 2)");
        assert_eq!(p.title, "Metal Gear Solid: The Twin Snakes");
        assert_eq!(p.disc, Some(2));
        assert_eq!(t("Final Fantasy VII (USA) (Disc 1)").disc, Some(1));
        assert_eq!(t("Some Game (Disk 3 of 4)").disc, Some(3));
        assert_eq!(t("Some Game (CD2)").disc, Some(2));
        let p = t("Snatcher (USA) (Track 01)");
        assert_eq!((p.title.as_str(), p.disc), ("Snatcher", None));
        // Discs of one game share a group key; different editions don't.
        assert_eq!(
            t("Final Fantasy VII (USA) (Disc 1)").group_key(),
            t("Final Fantasy VII (USA) (Disc 3)").group_key()
        );
        assert_ne!(
            t("Game (Disc 1)").group_key(),
            t("Game (Hack X) (Disc 2)").group_key()
        );
    }

    #[test]
    fn odd_names_degrade_gracefully() {
        title("v1_patched", "v1 patched");
        title("Pokemon - Radical Red", "Pokemon: Radical Red");
        title("Fire Emblem 4 English (Gharnef)", "Fire Emblem 4 English");
        title("Serenade Under the Moon ENG Fix v2.1.5 + Jonata Recolor v2.0", "Serenade Under the Moon ENG Fix v2.1.5 + Jonata Recolor v2.0");
        title("(USA)", "(USA)");
        // 3-digit numbers are titles, not release numbers.
        title("007 - Nightfire (USA)", "007: Nightfire");
    }

    #[test]
    fn search_keys_and_sorting() {
        assert_eq!(search_key("Luigi's Mansion"), "luigis mansion");
        assert_eq!(search_key("Pokémon: Platinum"), "pokemon platinum");
        assert_eq!(search_key("Kane & Lynch 2: Dog Days"), "kane and lynch 2 dog days");
        assert_eq!(search_key("  --Ys--  "), "ys");
        assert!(sort_title("Sly 2") < sort_title("Sly 10"));
        assert_eq!(sort_title("The Legend of Zelda"), "legend of zelda");
    }
}
