//! Matches a library game to a name in a libretro thumbnail index.
//!
//! 1. Exact: the file (or folder) name, sanitized, is in the index. No-Intro /
//!    Redump-named dumps hit this almost always.
//! 2. Same title: index names whose normalized search key equals the game's,
//!    preferring the game's region, then USA > World > Europe > Japan.
//! 3. Fuzzy: word-set overlap (Dice >= 0.8, ignoring "the/a/of/and") for scene
//!    names and loose titles ("Castlevania Dracula X Chronicles" ->
//!    "Castlevania - The Dracula X Chronicles (USA)").

use std::collections::{HashMap, HashSet};

use crate::scanner::normalize::{self, ParsedName};

use super::libretro::sanitize;

const STOPWORDS: &[&str] = &["the", "a", "an", "of", "and"];
pub const FUZZY_MIN: f32 = 0.8;

fn words(key: &str) -> HashSet<String> {
    key.split(' ').filter(|w| !w.is_empty() && !STOPWORDS.contains(w)).map(str::to_string).collect()
}

/// Title key ignoring little words, so "Castlevania - The Dracula X
/// Chronicles (USA)" and "Castlevania - Dracula X Chronicles (Japan)" count as
/// the same title and region preference decides between them.
fn title_key(search_key: &str) -> String {
    search_key.split(' ').filter(|w| !w.is_empty() && !STOPWORDS.contains(w)).collect::<Vec<_>>().join(" ")
}

/// Sequel markers: digits and roman numerals. Fuzzy matches must agree on
/// these, so "Final Fantasy X 3" never matches "Final Fantasy X".
fn numbers(w: &HashSet<String>) -> HashSet<&str> {
    const ROMAN: &[&str] = &["ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii"];
    w.iter()
        .map(String::as_str)
        .filter(|t| t.bytes().all(|b| b.is_ascii_digit()) || ROMAN.contains(t))
        .collect()
}

/// True if two words are equal, or long enough that a one-letter typo is
/// clearly a typo ("initital" ~ "initial", "resurreccion" ~ "resurrection").
fn same_word(a: &str, b: &str) -> bool {
    a == b || (a.len().min(b.len()) >= 5 && a.len().abs_diff(b.len()) <= 1 && edit_distance_le1(a, b))
}

fn edit_distance_le1(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut i = 0;
    while i < short.len() && short[i] == long[i] {
        i += 1;
    }
    if short.len() == long.len() {
        short[i + 1..] == long[i + 1..] // one substitution
    } else {
        short[i..] == long[i + 1..] // one insertion
    }
}

/// Dice coefficient over word sets, with typo-tolerant word equality.
fn dice(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let shared = a.iter().filter(|x| b.contains(*x) || b.iter().any(|y| same_word(x, y))).count();
    2.0 * shared as f32 / (a.len() + b.len()) as f32
}

/// Title with all spaces removed: "dragonball z budokai" == "dragon ball z budokai".
fn compact_key(search_key: &str) -> String {
    title_key(search_key).replace(' ', "")
}

pub struct Index {
    names: Vec<String>,
    parsed: Vec<ParsedName>,
    words: Vec<HashSet<String>>,
    exact: HashMap<String, usize>,
    by_key: HashMap<String, Vec<usize>>,
    by_compact: HashMap<String, Vec<usize>>,
}

/// What we know about the game being matched.
pub struct Query<'a> {
    /// Raw names to try verbatim: file stem, folder name.
    pub names: Vec<String>,
    pub search_key: &'a str,
    pub region: Option<&'a str>,
}

#[derive(Debug, PartialEq)]
pub struct Match<'a> {
    pub name: &'a str,
    /// 1.0 for exact/same-title matches, the Dice score for fuzzy ones.
    pub score: f32,
}

impl Index {
    pub fn new(names: Vec<String>) -> Self {
        let parsed: Vec<ParsedName> = names.iter().map(|n| normalize::parse(n)).collect();
        let words = parsed.iter().map(|p| words(&p.search_key)).collect();
        let exact = names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_compact: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, p) in parsed.iter().enumerate() {
            by_key.entry(title_key(&p.search_key)).or_default().push(i);
            by_compact.entry(compact_key(&p.search_key)).or_default().push(i);
        }
        Self { names, parsed, words, exact, by_key, by_compact }
    }

    fn pick(&self, cands: &[usize], region: Option<&str>) -> Option<usize> {
        cands.iter().copied().min_by_key(|&i| (self.rank(i, region), self.names[i].len()))
    }

    /// Lower is better: the game's own region first, then common English
    /// releases; demos/betas/hacks and later discs lose ties.
    fn rank(&self, i: usize, want_region: Option<&str>) -> u32 {
        let p = &self.parsed[i];
        let region = p.region.as_deref().unwrap_or("").to_ascii_lowercase();
        let mut r = match want_region {
            Some(w) if !w.is_empty() && region.contains(&w.to_ascii_lowercase()) => 0,
            _ if region.contains("usa") => 1,
            _ if region.contains("world") => 2,
            _ if region.contains("europe") => 3,
            _ if region.contains("japan") => 4,
            _ => 5,
        };
        if p.variant.is_some() {
            r += 10;
        }
        if p.disc.is_some_and(|d| d > 1) {
            r += 5;
        }
        r
    }

    pub fn best<'s>(&'s self, q: &Query) -> Option<Match<'s>> {
        for n in &q.names {
            if let Some(&i) = self.exact.get(&sanitize(n)) {
                return Some(Match { name: &self.names[i], score: 1.0 });
            }
        }
        let same_title = self.by_key.get(&title_key(q.search_key));
        let same_letters = || self.by_compact.get(&compact_key(q.search_key));
        if let Some(i) = same_title.or_else(same_letters).and_then(|c| self.pick(c, q.region)) {
            return Some(Match { name: &self.names[i], score: 1.0 });
        }
        let qw = words(q.search_key);
        if qw.is_empty() {
            return None;
        }
        let qn = numbers(&qw);
        let mut best: Option<(f32, u32, usize)> = None;
        for (i, w) in self.words.iter().enumerate() {
            let s = dice(&qw, w);
            if s < FUZZY_MIN || numbers(w) != qn {
                continue;
            }
            let rank = self.rank(i, q.region);
            let better = match best {
                None => true,
                Some((bs, br, bi)) => {
                    s > bs + 1e-6 || ((s - bs).abs() < 1e-6 && (rank, self.names[i].len()) < (br, self.names[bi].len()))
                }
            };
            if better {
                best = Some((s, rank, i));
            }
        }
        best.map(|(s, _, i)| Match { name: &self.names[i], score: s })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        Index::new(
            [
                "Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)",
                "Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v2.01)",
                "Jak II (Europe) (En,Fr,De,Es,It)",
                "Final Fantasy X (Japan)",
                "Final Fantasy X (Europe, Australia)",
                "Final Fantasy X (USA)",
                "Final Fantasy X-2 (USA)",
                "Final Fantasy X International (Japan)",
                "Castlevania - The Dracula X Chronicles (USA)",
                "Castlevania - Dracula X Chronicles (Japan)",
                "Kane _ Lynch 2 - Dog Days (USA)",
                "Final Fantasy VII (USA) (Disc 2)",
                "Final Fantasy VII (USA) (Disc 1)",
                "Shadow of the Colossus (USA)",
                "Shadow of the Colossus (USA) (Demo)",
                "Persona Q - Shadow of the Labyrinth (USA)",
                "inFAMOUS (USA)",
            ]
            .map(String::from)
            .to_vec(),
        )
    }

    fn q<'a>(names: &[&str], key: &'a str, region: Option<&'a str>) -> Query<'a> {
        Query { names: names.iter().map(|s| s.to_string()).collect(), search_key: key, region }
    }

    fn best(i: &Index, query: Query) -> Option<String> {
        i.best(&query).map(|m| m.name.to_string())
    }

    #[test]
    fn exact_file_name_wins() {
        let i = index();
        assert_eq!(
            best(&i, q(&["Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)"], "jak ii", Some("USA"))).as_deref(),
            Some("Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)")
        );
        // "&" in a file name maps to "_" in thumbnail names.
        assert_eq!(
            best(&i, q(&["Kane & Lynch 2 - Dog Days (USA)"], "kane and lynch 2 dog days", Some("USA"))).as_deref(),
            Some("Kane _ Lynch 2 - Dog Days (USA)")
        );
    }

    #[test]
    fn same_title_prefers_own_region_then_usa_and_no_demo() {
        let i = index();
        assert_eq!(best(&i, q(&["ffx"], "final fantasy x", Some("Europe"))).as_deref(), Some("Final Fantasy X (Europe, Australia)"));
        assert_eq!(best(&i, q(&["ffx"], "final fantasy x", None)).as_deref(), Some("Final Fantasy X (USA)"));
        assert_eq!(best(&i, q(&[], "shadow of the colossus", None)).as_deref(), Some("Shadow of the Colossus (USA)"));
        assert_eq!(best(&i, q(&[], "final fantasy vii", Some("USA"))).as_deref(), Some("Final Fantasy VII (USA) (Disc 1)"));
        // Case and punctuation don't matter (PS3 SFO titles).
        assert_eq!(best(&i, q(&[], "infamous", None)).as_deref(), Some("inFAMOUS (USA)"));
    }

    #[test]
    fn fuzzy_handles_scene_names_but_not_different_games() {
        let i = index();
        let m = i.best(&q(&["Castlevania_Dracula_X_Chronicles_USA"], "castlevania dracula x chronicles", Some("USA"))).unwrap();
        assert_eq!(m.name, "Castlevania - The Dracula X Chronicles (USA)");
        assert!(m.score >= FUZZY_MIN && m.score <= 1.0);
        // Spacing differences and typos in file names.
        let i2 = Index::new(
            ["Dragon Ball Z - Budokai Tenkaichi 3 (USA)", "killer7 (USA)", "Initial D - Street Stage (Japan)", "Initial D - Special Stage (Japan)"]
                .map(String::from)
                .to_vec(),
        );
        assert_eq!(best(&i2, q(&[], "dragonball z budokai tenkaichi 3", Some("USA"))).as_deref(), Some("Dragon Ball Z - Budokai Tenkaichi 3 (USA)"));
        assert_eq!(best(&i2, q(&[], "killer 7", Some("USA"))).as_deref(), Some("killer7 (USA)"));
        assert_eq!(best(&i2, q(&[], "initital d street stage", Some("Japan"))).as_deref(), Some("Initial D - Street Stage (Japan)"));
        // ...but a typo allowance never bridges different short words.
        assert!(!same_word("jak", "jax"));
        assert!(same_word("resurreccion", "resurrection"));
        assert!(!same_word("street", "special"));
        // A sequel or different game must not match.
        assert_eq!(best(&i, q(&[], "final fantasy x 3", None)), None);
        assert_eq!(best(&i, q(&[], "v1 patched", None)), None);
        assert_eq!(best(&i, q(&[], "persona q2 new cinema labyrinth", None)), None);
    }
}
