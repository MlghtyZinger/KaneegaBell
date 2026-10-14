//! Reads LaunchBox's `Metadata.zip` as it downloads: the zip's first entry,
//! `Metadata.xml` (~490 MB of XML, ~100 MB compressed), is inflated and parsed
//! on the fly and handed over one record at a time, so nothing big is ever
//! held in memory or written to disk.
//!
//! The XML is a flat list under one root element:
//! `<Game>` (all games first), then `<GameAlternateName>`, then `<GameImage>`,
//! plus platform/emulator records KB ignores.

use std::io::{BufReader, Read};

use flate2::read::DeflateDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LbGame {
    pub db_id: i64,
    pub name: String,
    pub platform: String,
    /// "2005-11-15" (or a longer ISO timestamp).
    pub release_date: Option<String>,
    pub release_year: Option<i32>,
    pub overview: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    /// "Action; Adventure"
    pub genres: Option<String>,
    pub max_players: Option<i64>,
    pub rating: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    Game(LbGame),
    AltName { db_id: i64, name: String, region: Option<String> },
    Image { db_id: i64, file: String, kind: String, region: Option<String> },
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Game,
    Alt,
    Image,
}

/// Skips the zip's local file header and returns a reader over the inflated
/// first entry, which must be `Metadata.xml`.
pub fn open_entry<R: Read>(mut zip: R) -> Result<impl Read> {
    let mut header = [0u8; 30];
    zip.read_exact(&mut header)?;
    let u16_at = |i: usize| u16::from_le_bytes([header[i], header[i + 1]]);
    if header[..4] != [0x50, 0x4b, 0x03, 0x04] {
        return Err(Error::msg("LaunchBox's download isn't a zip file"));
    }
    if u16_at(8) != 8 {
        return Err(Error::msg("LaunchBox's zip uses an unexpected compression"));
    }
    let mut name = vec![0u8; u16_at(26) as usize];
    zip.read_exact(&mut name)?;
    if name != b"Metadata.xml" {
        return Err(Error::msg(format!(
            "LaunchBox's zip starts with {} instead of Metadata.xml",
            String::from_utf8_lossy(&name)
        )));
    }
    std::io::copy(&mut (&mut zip).take(u16_at(28) as u64), &mut std::io::sink())?;
    Ok(DeflateDecoder::new(zip))
}

/// The fields KB uses; everything else is skipped without being copied.
fn wanted(kind: Kind, field: &str) -> bool {
    match kind {
        Kind::Game => matches!(
            field,
            "DatabaseID"
                | "Name"
                | "Platform"
                | "ReleaseDate"
                | "ReleaseYear"
                | "Overview"
                | "Developer"
                | "Publisher"
                | "Genres"
                | "MaxPlayers"
                | "CommunityRating"
        ),
        Kind::Alt => matches!(field, "DatabaseID" | "AlternateName" | "Region"),
        Kind::Image => matches!(field, "DatabaseID" | "FileName" | "Type" | "Region"),
    }
}

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => return None,
    })
}

fn build(kind: Kind, fields: &mut Vec<(&'static str, String)>) -> Option<Record> {
    let mut take = |f: &str| fields.iter().position(|(k, _)| *k == f).map(|i| fields.swap_remove(i).1);
    let db_id = take("DatabaseID")?.trim().parse().ok()?;
    let nonempty = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Some(match kind {
        Kind::Game => Record::Game(LbGame {
            db_id,
            name: nonempty(take("Name"))?,
            platform: nonempty(take("Platform"))?,
            release_date: nonempty(take("ReleaseDate")),
            release_year: take("ReleaseYear").and_then(|s| s.trim().parse().ok()),
            overview: nonempty(take("Overview")),
            developer: nonempty(take("Developer")),
            publisher: nonempty(take("Publisher")),
            genres: nonempty(take("Genres")),
            max_players: take("MaxPlayers").and_then(|s| s.trim().parse().ok()),
            rating: take("CommunityRating").and_then(|s| s.trim().parse().ok()),
        }),
        Kind::Alt => Record::AltName { db_id, name: nonempty(take("AlternateName"))?, region: nonempty(take("Region")) },
        Kind::Image => Record::Image {
            db_id,
            file: nonempty(take("FileName"))?,
            kind: nonempty(take("Type"))?,
            region: nonempty(take("Region")),
        },
    })
}

fn field_name(name: &str) -> &'static str {
    match name {
        "DatabaseID" => "DatabaseID",
        "Name" => "Name",
        "Platform" => "Platform",
        "ReleaseDate" => "ReleaseDate",
        "ReleaseYear" => "ReleaseYear",
        "Overview" => "Overview",
        "Developer" => "Developer",
        "Publisher" => "Publisher",
        "Genres" => "Genres",
        "MaxPlayers" => "MaxPlayers",
        "CommunityRating" => "CommunityRating",
        "AlternateName" => "AlternateName",
        "Region" => "Region",
        "FileName" => "FileName",
        "Type" => "Type",
        _ => "",
    }
}

/// Parses the XML, calling `on` for every game, alternate title and image.
/// `on` returning an error stops the parse (e.g. when cancelled).
pub fn records<R: Read>(xml: R, mut on: impl FnMut(Record) -> Result<()>) -> Result<()> {
    let mut reader = Reader::from_reader(BufReader::with_capacity(256 * 1024, xml));
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut depth = 0usize;
    let mut kind: Option<Kind> = None;
    let mut field: Option<&'static str> = None;
    let mut text = String::new();
    let mut fields: Vec<(&'static str, String)> = Vec::with_capacity(12);
    let bad = |e: quick_xml::Error| Error::msg(format!("LaunchBox's data couldn't be read: {e}"));
    loop {
        match reader.read_event_into(&mut buf).map_err(bad)? {
            Event::Start(e) => {
                depth += 1;
                match depth {
                    2 => {
                        kind = match e.name().as_ref() {
                            "Game" => Some(Kind::Game),
                            "GameAlternateName" => Some(Kind::Alt),
                            "GameImage" => Some(Kind::Image),
                            _ => None,
                        };
                        fields.clear();
                    }
                    3 => {
                        field = kind.filter(|k| wanted(*k, e.name().as_ref())).map(|_| field_name(e.name().as_ref()));
                        text.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(t) if field.is_some() && depth == 3 => text.push_str(&t.xml10_content()),
            Event::CData(t) if field.is_some() && depth == 3 => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) if field.is_some() && depth == 3 => {
                let name = r.xml10_content();
                match r.resolve_char_ref().ok().flatten().or_else(|| entity(&name)) {
                    Some(c) => text.push(c),
                    None => {
                        text.push('&');
                        text.push_str(&name);
                        text.push(';');
                    }
                }
            }
            Event::End(_) => {
                if depth == 3 {
                    if let Some(f) = field.take() {
                        fields.push((f, std::mem::take(&mut text)));
                    }
                } else if depth == 2 {
                    if let Some(k) = kind.take() {
                        if let Some(r) = build(k, &mut fields) {
                            on(r)?;
                        }
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    pub const SAMPLE: &str = r#"<?xml version="1.0" standalone="yes"?>
<LaunchBox>
  <Platform><Name>Sony Playstation 2</Name></Platform>
  <Game>
    <Name>Jak &amp; Daxter: The Precursor Legacy</Name>
    <ReleaseDate>2001-12-03T00:00:00-08:00</ReleaseDate>
    <ReleaseYear>2001</ReleaseYear>
    <Overview>Jak &amp; Daxter &#8211; a platformer.</Overview>
    <MaxPlayers>1</MaxPlayers>
    <DatabaseID>101</DatabaseID>
    <CommunityRating>4.2</CommunityRating>
    <Platform>Sony Playstation 2</Platform>
    <Genres>Platform; Adventure</Genres>
    <Developer>Naughty Dog</Developer>
    <Publisher>Sony Computer Entertainment</Publisher>
    <VideoURL>https://example.com/v</VideoURL>
  </Game>
  <Game>
    <Name>Okami</Name>
    <DatabaseID>102</DatabaseID>
    <Platform>Sony Playstation 2</Platform>
    <Overview />
  </Game>
  <Game>
    <Name>Some PC Game</Name>
    <DatabaseID>900</DatabaseID>
    <Platform>Windows</Platform>
  </Game>
  <GameAlternateName>
    <AlternateName>Ookami</AlternateName>
    <DatabaseID>102</DatabaseID>
    <Region>Japan</Region>
  </GameAlternateName>
  <GameImage>
    <DatabaseID>101</DatabaseID>
    <FileName>aaaa-01.jpg</FileName>
    <Type>Box - Front</Type>
    <Region>North America</Region>
    <CRC32>123</CRC32>
  </GameImage>
  <GameImage>
    <DatabaseID>101</DatabaseID>
    <FileName>bbbb-01.png</FileName>
    <Type>Clear Logo</Type>
  </GameImage>
</LaunchBox>
"#;

    /// A zip whose first entry is `Metadata.xml` holding `xml`.
    pub fn zip_of(xml: &str) -> Vec<u8> {
        let mut deflated = Vec::new();
        let mut enc = flate2::write::DeflateEncoder::new(&mut deflated, flate2::Compression::fast());
        enc.write_all(xml.as_bytes()).unwrap();
        enc.finish().unwrap();
        let name = b"Metadata.xml";
        let mut zip = Vec::new();
        zip.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0x08, 0, 8, 0]); // sig, version, flags (data descriptor), deflate
        zip.extend_from_slice(&[0; 16]); // time, date, crc, sizes (in the data descriptor)
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&3u16.to_le_bytes());
        zip.extend_from_slice(name);
        zip.extend_from_slice(b"xyz"); // extra field
        zip.extend_from_slice(&deflated);
        zip.extend_from_slice(b"PK\x07\x08 trailing entries are never read");
        zip
    }

    #[test]
    fn streams_games_alternate_names_and_images_from_the_zip() {
        let zip = zip_of(SAMPLE);
        let mut got = Vec::new();
        records(open_entry(&zip[..]).unwrap(), |r| {
            got.push(r);
            Ok(())
        })
        .unwrap();
        assert_eq!(got.len(), 6);
        let Record::Game(jak) = &got[0] else { panic!("{:?}", got[0]) };
        assert_eq!(jak.name, "Jak & Daxter: The Precursor Legacy");
        assert_eq!(jak.overview.as_deref(), Some("Jak & Daxter – a platformer."));
        assert_eq!((jak.db_id, jak.release_year, jak.max_players, jak.rating), (101, Some(2001), Some(1), Some(4.2)));
        assert_eq!(jak.genres.as_deref(), Some("Platform; Adventure"));
        let Record::Game(okami) = &got[1] else { panic!() };
        assert_eq!((okami.name.as_str(), okami.overview.clone()), ("Okami", None));
        assert_eq!(got[3], Record::AltName { db_id: 102, name: "Ookami".into(), region: Some("Japan".into()) });
        assert_eq!(
            got[4],
            Record::Image { db_id: 101, file: "aaaa-01.jpg".into(), kind: "Box - Front".into(), region: Some("North America".into()) }
        );
        assert_eq!(got[5], Record::Image { db_id: 101, file: "bbbb-01.png".into(), kind: "Clear Logo".into(), region: None });
    }

    #[test]
    fn a_stop_from_the_callback_ends_the_parse() {
        let zip = zip_of(SAMPLE);
        let mut n = 0;
        let r = records(open_entry(&zip[..]).unwrap(), |_| {
            n += 1;
            Err(Error::msg("cancelled"))
        });
        assert!(r.is_err());
        assert_eq!(n, 1);
    }

    #[test]
    fn wrong_files_are_refused() {
        assert!(open_entry(&b"<html>not a zip</html>-------------------------"[..]).is_err());
        let mut zip = zip_of(SAMPLE);
        zip[30..42].copy_from_slice(b"Platform.xml");
        assert!(open_entry(&zip[..]).is_err());
    }
}
