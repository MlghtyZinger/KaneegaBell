//! Matches library games to LaunchBox names on the same platform, by the same
//! title rules as the cover matcher:
//! 1. Same title (ignoring little words, punctuation and spacing). A main
//!    name beats an alternate title; among alternate titles, the one used in
//!    the game's region wins ("Ookami" for a Japanese dump).
//! 2. Close title (word overlap >= 0.8, sequel numbers must agree).
//! When different LaunchBox games are equally good, nothing is guessed: the
//! game is left for Fix match.

use std::collections::{HashMap, HashSet};

use crate::art::matcher::{compact_key, dice, numbers, title_key, words, FUZZY_MIN};
use crate::scanner::normalize;

#[derive(Debug, Clone, PartialEq)]
pub struct Name {
    pub db_id: i64,
    pub name: String,
    pub region: Option<String>,
    pub is_alt: bool,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Match(i64),
    /// Several different LaunchBox games fit equally well.
    Ambiguous,
    NotFound,
}

/// A Fix match search result.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub db_id: i64,
    /// The game's main LaunchBox name.
    pub name: String,
    /// The alternate title that matched, if that's what matched.
    pub also_known_as: Option<String>,
    pub score: f32,
}

/// The names of one platform, prepared for matching.
pub struct Candidates {
    names: Vec<Name>,
    words: Vec<HashSet<String>>,
    by_title: HashMap<String, Vec<usize>>,
    by_compact: HashMap<String, Vec<usize>>,
    main_name: HashMap<i64, usize>,
}

/// Does a LaunchBox region ("North America", "Japan", ...) fit the game's
/// region from its file name ("USA", "Europe, Australia", ...)?
pub fn region_fits(game_region: Option<&str>, lb_region: Option<&str>) -> bool {
    let (Some(g), Some(l)) = (game_region, lb_region) else { return false };
    let (g, l) = (g.to_ascii_lowercase(), l.to_ascii_lowercase());
    let any = |list: &[&str], s: &str| list.iter().any(|x| s.contains(x));
    const NA: &[&str] = &["usa", "united states", "north america", "canada", "america"];
    const EU: &[&str] = &["europe", "united kingdom", "uk", "germany", "france", "spain", "italy", "australia", "netherlands", "sweden", "pal"];
    (any(NA, &g) && any(NA, &l)) || (any(EU, &g) && any(EU, &l)) || (g.contains("japan") && l.contains("japan")) || l.contains(&g)
}

impl Candidates {
    pub fn new(names: Vec<Name>) -> Self {
        let keys: Vec<String> = names.iter().map(|n| normalize::search_key(&n.name)).collect();
        let words = keys.iter().map(|k| words(k)).collect();
        let mut by_title: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_compact: HashMap<String, Vec<usize>> = HashMap::new();
        let mut main_name = HashMap::new();
        for (i, k) in keys.iter().enumerate() {
            by_title.entry(title_key(k)).or_default().push(i);
            by_compact.entry(compact_key(k)).or_default().push(i);
            if !names[i].is_alt {
                main_name.insert(names[i].db_id, i);
            }
        }
        Self { names, words, by_title, by_compact, main_name }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// A LaunchBox game's main name.
    pub fn main_name(&self, db_id: i64) -> Option<&str> {
        self.main_name.get(&db_id).map(|&i| self.names[i].name.as_str())
    }

    /// Lower is better: a main name, then an alternate title from the game's
    /// region, then any other alternate title.
    fn rank(&self, i: usize, region: Option<&str>) -> u8 {
        let n = &self.names[i];
        if !n.is_alt {
            0
        } else if region_fits(region, n.region.as_deref()) {
            1
        } else {
            2
        }
    }

    /// The single best entry among `cands` (best rank per game), or Ambiguous
    /// when two different games share the best rank.
    fn decide(&self, cands: impl Iterator<Item = usize>, region: Option<&str>) -> Outcome {
        let mut best_by_game: HashMap<i64, u8> = HashMap::new();
        for i in cands {
            let r = self.rank(i, region);
            let e = best_by_game.entry(self.names[i].db_id).or_insert(r);
            *e = (*e).min(r);
        }
        let Some(&top) = best_by_game.values().min() else { return Outcome::NotFound };
        let mut winners = best_by_game.iter().filter(|(_, r)| **r == top).map(|(id, _)| *id);
        match (winners.next(), winners.next()) {
            (Some(id), None) => Outcome::Match(id),
            (Some(_), Some(_)) => Outcome::Ambiguous,
            _ => Outcome::NotFound,
        }
    }

    pub fn best(&self, search_key: &str, region: Option<&str>) -> Outcome {
        let exact: Vec<usize> = self
            .by_title
            .get(&title_key(search_key))
            .into_iter()
            .chain(self.by_compact.get(&compact_key(search_key)))
            .flatten()
            .copied()
            .collect();
        if !exact.is_empty() {
            return self.decide(exact.into_iter(), region);
        }
        let qw = words(search_key);
        if qw.is_empty() {
            return Outcome::NotFound;
        }
        let qn = numbers(&qw);
        let mut best = 0.0f32;
        let mut tied: Vec<usize> = Vec::new();
        for (i, w) in self.words.iter().enumerate() {
            let s = dice(&qw, w);
            if s < FUZZY_MIN || numbers(w) != qn {
                continue;
            }
            if s > best + 1e-6 {
                best = s;
                tied.clear();
            }
            if (s - best).abs() < 1e-6 {
                tied.push(i);
            }
        }
        self.decide(tied.into_iter(), region)
    }

    /// Fix match: entries for a typed title (or the game's own), best first,
    /// one per LaunchBox game. Sequel numbers aren't enforced: the user decides.
    pub fn search(&self, query_key: &str, limit: usize) -> Vec<Hit> {
        let tk = title_key(query_key);
        let ck = compact_key(query_key);
        let qw = words(query_key);
        let mut best: HashMap<i64, (f32, usize)> = HashMap::new();
        for (i, n) in self.names.iter().enumerate() {
            let key = normalize::search_key(&n.name);
            let score = if title_key(&key) == tk || compact_key(&key) == ck {
                1.0
            } else if !tk.is_empty() && (title_key(&key).contains(&tk) || compact_key(&key).contains(&ck)) {
                0.9
            } else {
                let d = dice(&qw, &self.words[i]);
                if d < 0.4 {
                    continue;
                }
                d * 0.85
            };
            let e = best.entry(n.db_id).or_insert((score, i));
            if score > e.0 || (score == e.0 && !n.is_alt) {
                *e = (score, i);
            }
        }
        let mut hits: Vec<Hit> = best
            .into_iter()
            .map(|(db_id, (score, i))| {
                let main = self.main_name.get(&db_id).map(|&m| self.names[m].name.clone());
                let matched = &self.names[i];
                Hit {
                    db_id,
                    name: main.clone().unwrap_or_else(|| matched.name.clone()),
                    also_known_as: (matched.is_alt && main.is_some()).then(|| matched.name.clone()),
                    score,
                }
            })
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.name.cmp(&b.name)));
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(db_id: i64, name: &str, alt: Option<&str>) -> Name {
        Name { db_id, name: name.into(), region: alt.map(String::from), is_alt: alt.is_some() }
    }

    fn ps2() -> Candidates {
        Candidates::new(vec![
            n(1, "Jak and Daxter: The Precursor Legacy", None),
            n(2, "Okami", None),
            n(2, "Ookami", Some("Japan")),
            n(3, "Final Fantasy X", None),
            n(4, "Final Fantasy X-2", None),
            n(5, "Ratchet & Clank: Going Commando", None),
            n(5, "Ratchet & Clank 2", Some("Europe")),
            n(6, "Ratchet & Clank 2: Locked and Loaded", Some("Australia")),
            n(7, "Tetris Worlds", None),
            n(8, "Tetris Worlds", None),
            n(9, "Metal Gear Solid 3: Snake Eater", None),
        ])
    }

    fn key(s: &str) -> String {
        normalize::search_key(s)
    }

    #[test]
    fn same_title_by_main_name_or_alternate_title() {
        let c = ps2();
        assert_eq!(c.best(&key("Jak & Daxter - The Precursor Legacy"), Some("USA")), Outcome::Match(1));
        assert_eq!(c.best(&key("Ookami"), Some("Japan")), Outcome::Match(2));
        assert_eq!(c.best(&key("okami"), Some("USA")), Outcome::Match(2));
        assert_eq!(c.best(&key("Final Fantasy X"), None), Outcome::Match(3));
        // An alternate title from the dump's own region wins over another game's.
        assert_eq!(c.best(&key("Ratchet & Clank 2"), Some("Europe")), Outcome::Match(5));
    }

    #[test]
    fn close_titles_match_but_sequels_and_strangers_dont() {
        let c = ps2();
        assert_eq!(c.best(&key("Metal Gear Solid 3 Snake Eater"), Some("USA")), Outcome::Match(9));
        assert_eq!(c.best(&key("Final Fantasy X 3"), None), Outcome::NotFound);
        assert_eq!(c.best(&key("Shadow of the Colossus"), None), Outcome::NotFound);
    }

    #[test]
    fn two_different_games_with_the_same_name_are_left_for_fix_match() {
        assert_eq!(ps2().best(&key("Tetris Worlds"), None), Outcome::Ambiguous);
    }

    #[test]
    fn search_lists_one_entry_per_game_with_its_main_name() {
        let c = ps2();
        let hits = c.search(&key("ookami"), 5);
        assert_eq!(hits[0].db_id, 2);
        assert_eq!(hits[0].name, "Okami");
        assert_eq!(hits[0].also_known_as.as_deref(), Some("Ookami"));
        let ff: Vec<i64> = c.search(&key("final fantasy"), 5).iter().map(|h| h.db_id).collect();
        assert!(ff.contains(&3) && ff.contains(&4));
        assert_eq!(c.search(&key("ratchet"), 10).iter().filter(|h| h.db_id == 5).count(), 1);
    }

    #[test]
    fn regions_fit_across_naming_styles() {
        assert!(region_fits(Some("USA"), Some("North America")));
        assert!(region_fits(Some("Europe, Australia"), Some("United Kingdom")));
        assert!(region_fits(Some("Japan"), Some("Japan")));
        assert!(!region_fits(Some("USA"), Some("Japan")));
        assert!(!region_fits(None, Some("Japan")));
    }
}
