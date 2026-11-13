//! Groups the discs of one game into a single library entry.
//!
//! Files in the same directory, on the same platform, whose names parse to the
//! same title+variant and carry a disc number are one game. A real .m3u always
//! wins: its discs were already hidden as companions before grouping runs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::normalize::ParsedName;

#[derive(Debug, Clone)]
pub struct Item {
    pub platform: String,
    pub path: PathBuf,
    pub parsed: ParsedName,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub platform: String,
    /// Parsed name of the first disc (disc number set) or of the single file.
    pub parsed: ParsedName,
    /// Sorted by disc number; one element for single-file games.
    pub discs: Vec<(PathBuf, Option<u32>)>,
}

impl Group {
    pub fn is_multidisc(&self) -> bool {
        self.discs.len() > 1
    }
    pub fn launch_path(&self) -> &PathBuf {
        &self.discs[0].0
    }
}

pub fn group(items: Vec<Item>) -> Vec<Group> {
    let mut singles = Vec::new();
    // Key: (dir, platform, group_key). BTreeMap keeps output order stable.
    let mut sets: BTreeMap<(PathBuf, String, String), Vec<Item>> = BTreeMap::new();

    for item in items {
        if item.parsed.disc.is_some() {
            let dir = item.path.parent().map(PathBuf::from).unwrap_or_default();
            sets.entry((dir, item.platform.clone(), item.parsed.group_key()))
                .or_default()
                .push(item);
        } else {
            singles.push(item);
        }
    }

    let mut out: Vec<Group> = singles
        .into_iter()
        .map(|i| Group { platform: i.platform, discs: vec![(i.path, None)], parsed: i.parsed })
        .collect();

    for (_, mut members) in sets {
        members.sort_by(|a, b| a.parsed.disc.cmp(&b.parsed.disc).then_with(|| a.path.cmp(&b.path)));
        let first = members[0].clone();
        let discs = if members.len() == 1 {
            // A lone "(Disc 1)" file is just a normal game.
            vec![(first.path.clone(), None)]
        } else {
            members.into_iter().map(|m| (m.path, m.parsed.disc)).collect()
        };
        out.push(Group { platform: first.platform, parsed: first.parsed, discs });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::normalize::parse;

    fn item(platform: &str, path: &str) -> Item {
        let p = PathBuf::from(path);
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        Item { platform: platform.into(), parsed: parse(&stem), path: p }
    }

    #[test]
    fn ff7_three_cues_become_one_game() {
        let d = r"E:\PS1\Final Fantasy VII (USA) (Disc 1 & 2 & 3)";
        let groups = group(vec![
            item("ps1", &format!(r"{d}\Final Fantasy VII (USA) (Disc 3).cue")),
            item("ps1", &format!(r"{d}\Final Fantasy VII (USA) (Disc 1).cue")),
            item("ps1", &format!(r"{d}\Final Fantasy VII (USA) (Disc 2).cue")),
            item("ps1", r"E:\PS1\Silent Hill (USA)\Silent Hill (USA).cue"),
        ]);
        assert_eq!(groups.len(), 2);
        let ff7 = groups.iter().find(|g| g.is_multidisc()).unwrap();
        assert_eq!(ff7.parsed.title, "Final Fantasy VII");
        assert_eq!(ff7.discs.iter().map(|d| d.1).collect::<Vec<_>>(), [Some(1), Some(2), Some(3)]);
        assert!(ff7.launch_path().ends_with("Final Fantasy VII (USA) (Disc 1).cue"));
    }

    #[test]
    fn twin_snakes_two_isos_become_one_game() {
        let d = r"E:\GC\Metal gear solid - The Twin Snakes";
        let groups = group(vec![
            item("gc", &format!(r"{d}\Metal Gear Solid - The Twin Snakes (Disc 1).iso")),
            item("gc", &format!(r"{d}\Metal Gear Solid - The Twin Snakes (Disc 2).iso")),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].parsed.title, "Metal Gear Solid: The Twin Snakes");
        assert_eq!(groups[0].discs.len(), 2);
    }

    #[test]
    fn different_dirs_platforms_or_variants_are_not_merged() {
        let groups = group(vec![
            item("ps1", r"A\Game (Disc 1).cue"),
            item("ps1", r"B\Game (Disc 2).cue"),
            item("ps1", r"C\Game (Disc 1).cue"),
            item("segacd", r"C\Game (Disc 2).cue"),
            item("ps1", r"D\Game (Disc 1).cue"),
            item("ps1", r"D\Game (Undub) (Disc 2).cue"),
        ]);
        assert_eq!(groups.len(), 6);
        assert!(groups.iter().all(|g| !g.is_multidisc()));
        assert!(groups.iter().all(|g| g.discs[0].1.is_none()));
    }
}
