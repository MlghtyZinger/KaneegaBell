//! libretro-thumbnails (thumbnails.libretro.com): keyless box art, organized
//! as `{System}/Named_Boxarts/{Game Name}.png`. Game names follow No-Intro /
//! Redump naming, with `& * / : \` < > ? \ | "` replaced by `_`.

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use regex::Regex;
use std::sync::LazyLock;

pub const BASE: &str = "https://thumbnails.libretro.com";

/// Characters escaped in URL path segments (everything a server could misread).
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'+')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'^')
    .add(b'\\');

/// Applies libretro's thumbnail-name rule.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '&' | '*' | '/' | ':' | '`' | '<' | '>' | '?' | '\\' | '|' | '"') { '_' } else { c })
        .collect()
}

fn seg(s: &str) -> String {
    utf8_percent_encode(s, SEGMENT).to_string()
}

pub fn index_url(system: &str) -> String {
    format!("{BASE}/{}/Named_Boxarts/", seg(system))
}

pub fn boxart_url(system: &str, name: &str) -> String {
    format!("{BASE}/{}/Named_Boxarts/{}.png", seg(system), seg(name))
}

static HREF_PNG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"href="([^"?/][^"]*?)\.png""#).unwrap());

/// Game names in a `Named_Boxarts/` directory listing (Apache index HTML).
pub fn parse_index(html: &str) -> Vec<String> {
    let mut names: Vec<String> = HREF_PNG
        .captures_iter(html)
        .filter_map(|c| percent_decode_str(&c[1]).decode_utf8().ok().map(|s| s.replace("&amp;", "&")))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_like_retroarch() {
        assert_eq!(sanitize("Q*bert's Qubes (USA)"), "Q_bert's Qubes (USA)");
        assert_eq!(sanitize("Kane & Lynch 2: Dog Days"), "Kane _ Lynch 2_ Dog Days");
        assert_eq!(sanitize("Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)"), "Jak II (USA) (En,Ja,Fr,De,Es,It,Ko) (v1.00)");
    }

    #[test]
    fn urls_escape_spaces_and_specials() {
        assert_eq!(
            index_url("Sony - PlayStation 2"),
            "https://thumbnails.libretro.com/Sony%20-%20PlayStation%202/Named_Boxarts/"
        );
        assert_eq!(
            boxart_url("Nintendo - GameCube", "Luigi's Mansion (USA)"),
            "https://thumbnails.libretro.com/Nintendo%20-%20GameCube/Named_Boxarts/Luigi%27s%20Mansion%20(USA).png"
        );
        assert!(boxart_url("X", "A+B #1").ends_with("A%2BB%20%231.png"));
    }

    #[test]
    fn parses_apache_listing() {
        let html = r#"<tr><td><a href="/Sony%20-%20PlayStation%202/">Parent Directory</a></td></tr>
<tr><td><a href="?C=N;O=D">Name</a></td></tr>
<tr><td><a href="Jak%20II%20(USA)%20(En%2CJa).png">Jak II (USA) (En,Ja).png</a></td></tr>
<tr><td><a href="Ratchet%20%26%20Clank%20(USA).png">Ratchet &amp; Clank (USA).png</a></td></tr>
<tr><td><a href="Shadow%20of%20the%20Colossus%20(USA).png">x</a></td></tr>
<tr><td><a href="Shadow%20of%20the%20Colossus%20(USA).png">dup</a></td></tr>"#;
        assert_eq!(
            parse_index(html),
            ["Jak II (USA) (En,Ja)", "Ratchet & Clank (USA)", "Shadow of the Colossus (USA)"]
        );
    }
}
