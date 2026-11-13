use super::*;
use serde_json::Value;
#[derive(Deserialize, Default)]
struct Inventory {
    client: Option<PathBuf>,
    #[serde(default)]
    games: Vec<Row>,
    #[serde(default)]
    warnings: Vec<String>,
    complete: bool,
}
#[derive(Deserialize)]
struct Row {
    id: String,
    name: String,
    dir: PathBuf,
    #[serde(default)]
    exes: Vec<String>,
}
fn inventory(provider: &str) -> Result<Inventory> {
    name(provider)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let script = format!(
            "& {{ param([string]$Provider)\n{}\n}} '{provider}'",
            include_str!("inventory.ps1")
        );
        let exe =
            PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        use std::{
            io::Read,
            process::Stdio,
            time::{Duration, Instant},
        };
        let mut child = std::process::Command::new(exe)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .creation_flags(0x08000000)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::msg("Launcher inventory output unavailable."))?;
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(Error::msg(
                    "Reading installed-game records timed out. Previous games were kept.",
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let bytes = reader
            .join()
            .map_err(|_| Error::msg("Launcher inventory reader failed."))??;
        if !status.success() || bytes.len() > 8 * 1024 * 1024 {
            return Err(Error::msg(format!(
                "{} installed-game records could not be read. Previous games were kept.",
                name(provider)?
            )));
        }
        return Ok(serde_json::from_slice(&bytes)?);
    }
    #[cfg(not(windows))]
    Err(Error::msg(
        "Launcher integrations currently require Windows.",
    ))
}
fn text<'a>(j: &'a Value, key: &str) -> Option<&'a str> {
    j.get(key).and_then(Value::as_str)
}
fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let rows = fs::read_dir(dir)?
        .take(4097)
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    if rows.len() > 4096 {
        return Err(Error::msg(
            "Too many launcher metadata entries; previous games were kept.",
        ));
    }
    Ok(rows)
}
fn client_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(unc) = s.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        s.strip_prefix("\\\\?\\").unwrap_or(&s).to_owned()
    }
}
fn directory(path: &Path) -> Result<PathBuf> {
    let dir = fs::canonicalize(path)?;
    if !dir.is_dir() {
        return Err(Error::msg("Game install directory is unavailable."));
    }
    Ok(dir)
}
fn data_root() -> PathBuf {
    PathBuf::from(std::env::var_os("PROGRAMDATA").unwrap_or_else(|| "C:\\ProgramData".into()))
}
fn collect(c: &mut Catalog, result: Result<Option<Installed>>) {
    match result {
        Ok(Some(g)) => c.games.push(g),
        Ok(None) => {}
        Err(_) => {
            c.complete = false;
            c.warnings.push("An installed-game record was unreadable or its drive was unavailable; previous games were kept.".into());
        }
    }
}

pub(super) fn epic(manifest: &Value, listed_dir: Option<&Path>) -> Result<Option<Installed>> {
    let id = identifier(
        text(manifest, "AppName").ok_or_else(|| Error::msg("Missing Epic product ID."))?,
    )?;
    if id.starts_with("UE_") || text(manifest, "CatalogNamespace") == Some("ue") {
        return Ok(None);
    }
    if manifest.get("bIsApplication").and_then(Value::as_bool) != Some(true)
        || manifest.get("bIsExecutable").and_then(Value::as_bool) != Some(true)
        || manifest
            .get("bIsIncompleteInstall")
            .and_then(Value::as_bool)
            == Some(true)
    {
        return Ok(None);
    }
    if text(manifest, "MainGameAppName").is_some_and(|m| !m.is_empty() && m != id) {
        return Ok(None);
    }
    let categories = manifest.get("AppCategories").and_then(Value::as_array);
    if categories.is_some_and(|c| {
        c.iter().any(|s| s.as_str() == Some("addons"))
            && !c.iter().any(|s| s.as_str() == Some("addons/launchable"))
    }) {
        return Ok(None);
    }
    if categories.is_some_and(|c| {
        c.iter().any(|s| {
            s.as_str()
                .is_some_and(|v| v == "apps" || v.starts_with("plugins"))
        })
    }) {
        return Ok(None);
    }
    let ns = identifier(
        text(manifest, "CatalogNamespace").ok_or_else(|| Error::msg("Missing Epic namespace."))?,
    )?;
    let item = identifier(
        text(manifest, "CatalogItemId").ok_or_else(|| Error::msg("Missing Epic catalog ID."))?,
    )?;
    let dir = directory(
        listed_dir.unwrap_or(Path::new(text(manifest, "InstallLocation").unwrap_or(""))),
    )?;
    let executables = text(manifest, "LaunchExecutable")
        .and_then(|p| candidate(&dir, p))
        .into_iter()
        .collect();
    let title = text(manifest, "DisplayName")
        .filter(|s| !s.is_empty() && s.len() < 1024)
        .ok_or_else(|| Error::msg("Missing Epic title."))?
        .to_owned();
    Ok(Some(Installed {
        id: id.into(),
        title,
        dir,
        executables,
        action: Action::Uri(format!(
            "com.epicgames.launcher://apps/{}%3A{}%3A{}?action=launch&silent=true",
            component(ns),
            component(item),
            component(id)
        )),
    }))
}
fn epic_catalog(root: &Path) -> Result<Catalog> {
    let list = json(&root.join("Epic/UnrealEngineLauncher/LauncherInstalled.dat"))?;
    let installs = list
        .get("InstallationList")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::msg("Unsupported Epic installation list."))?;
    let mut c = Catalog {
        complete: true,
        ..Default::default()
    };
    let dir = root.join("Epic/EpicGamesLauncher/Data/Manifests");
    if !dir.is_dir() {
        if installs
            .iter()
            .any(|a| text(a, "AppName").is_some_and(|v| !v.starts_with("UE_")))
        {
            c.complete = false;
            c.warnings.push(
                "Epic installed manifests are unavailable. Open the client and scan again.".into(),
            );
        }
        return Ok(c);
    }
    for file in entries(&dir)? {
        if file
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("item"))
        {
            continue;
        }
        let result = (|| {
            let m = json(&file)?;
            let listed = installs
                .iter()
                .find(|a| text(a, "AppName") == text(&m, "AppName"))
                .and_then(|a| text(a, "InstallLocation"))
                .map(Path::new);
            epic(&m, listed)
        })();
        collect(&mut c, result);
    }
    Ok(c)
}
pub(super) fn gog(
    id: &str,
    dir: &Path,
    client: &Path,
    manifest: &Value,
) -> Result<Option<Installed>> {
    if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
        return Err(Error::msg("Invalid GOG product ID."));
    }
    if text(manifest, "gameId") != Some(id) {
        return Err(Error::msg("GOG manifest ID mismatch."));
    }
    if text(manifest, "rootGameId").is_some_and(|root| root != id) {
        return Ok(None);
    }
    let tasks = manifest
        .get("playTasks")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::msg("GOG launch tasks missing."))?;
    if tasks.is_empty() {
        return Ok(None);
    }
    let dir = directory(dir)?;
    let executables = tasks
        .iter()
        .filter(|t| text(t, "type") == Some("FileTask"))
        .filter_map(|t| text(t, "path"))
        .filter_map(|p| candidate(&dir, p))
        .collect();
    let title = text(manifest, "name")
        .filter(|s| !s.is_empty() && s.len() < 1024)
        .ok_or_else(|| Error::msg("GOG title missing."))?
        .into();
    if !client.is_file()
        || !client
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("GalaxyClient.exe"))
    {
        return Err(Error::msg("GOG Galaxy client was not found."));
    }
    Ok(Some(Installed {
        id: id.into(),
        title,
        dir: dir.clone(),
        executables,
        action: Action::Client {
            exe: fs::canonicalize(client)?,
            args: vec![
                "/launchViaAutostart".into(),
                format!("/gameId={id}"),
                "/command=runGame".into(),
                format!("/path={}", client_path(&dir)),
            ],
        },
    }))
}
fn gog_catalog() -> Result<Catalog> {
    let i = inventory("gog")?;
    let mut c = Catalog {
        complete: i.complete,
        warnings: i.warnings,
        ..Default::default()
    };
    let client = i
        .client
        .ok_or_else(|| Error::msg("GOG Galaxy was not found."))?;
    for row in i.games {
        let r = (|| {
            gog(
                &row.id,
                &row.dir,
                &client,
                &json(
                    &row.dir
                        .join(format!("goggame-{}.info", identifier(&row.id)?)),
                )?,
            )
        })();
        collect(&mut c, r);
    }
    Ok(c)
}
fn ubisoft_catalog() -> Result<Catalog> {
    let i = inventory("ubisoft")?;
    let mut c = Catalog {
        complete: i.complete,
        warnings: i.warnings,
        ..Default::default()
    };
    for row in i.games {
        let result = (|| {
            let id = identifier(&row.id)?;
            if id.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
                return Err(Error::msg("Invalid Ubisoft game ID."));
            }
            Ok(Some(Installed {
                id: id.into(),
                title: row.name,
                dir: directory(&row.dir)?,
                executables: vec![],
                action: Action::Uri(format!("uplay://launch/{id}/0")),
            }))
        })();
        collect(&mut c, result);
    }
    Ok(c)
}
fn amazon_catalog(path: &Path) -> Result<Catalog> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(2))?;
    let mut stmt = conn.prepare(
        "SELECT Id,ProductTitle,InstallDirectory FROM DbSet WHERE Installed=1 LIMIT 4097",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if rows.len() > 4096 {
        return Err(Error::msg("Too many Amazon installed-game records."));
    }
    let mut c = Catalog {
        complete: true,
        ..Default::default()
    };
    for (id, title, dir) in rows {
        collect(
            &mut c,
            (|| {
                identifier(&id)?;
                let dir = directory(Path::new(&dir))?;
                let fuel = dir.join("fuel.json");
                let mut executables = vec![];
                if fuel.is_file() {
                    let fuel = json(&fuel)?;
                    if let Some(p) = fuel
                        .get("Main")
                        .and_then(|j| text(j, "Command"))
                        .and_then(|p| candidate(&dir, p))
                    {
                        executables.push(p);
                    }
                }
                Ok(Some(Installed {
                    id: id.clone(),
                    title,
                    dir,
                    executables,
                    action: Action::Uri(format!("amazon-games://play/{}", component(&id))),
                }))
            })(),
        );
    }
    Ok(c)
}
// Only the installed manifest fields are decoded; unknown query fields are discarded.
pub(super) fn ea_manifest(input: &str) -> Result<Option<Installed>> {
    let query = input
        .trim()
        .split_once('?')
        .map(|p| p.1)
        .unwrap_or(input.trim());
    let mut fields = std::collections::BTreeMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let k = k.to_ascii_lowercase();
            if [
                "id",
                "currentstate",
                "previousstate",
                "ddinstallalreadycompleted",
                "dipinstallpath",
            ]
            .contains(&k.as_str())
            {
                let value = percent_encoding::percent_decode_str(&v.replace('+', " "))
                    .decode_utf8()
                    .map_err(|_| Error::msg("Invalid EA manifest encoding."))?
                    .into_owned();
                if fields.insert(k, value).is_some() {
                    return Err(Error::msg("Duplicate EA manifest field."));
                }
            }
        }
    }
    let get = |k: &str| fields.get(k).map(String::as_str).unwrap_or("");
    if get("ddinstallalreadycompleted") != "1"
        && !(get("currentstate") == "kReadyToStart" && get("previousstate") == "kCompleted")
    {
        return Ok(None);
    }
    let id = identifier(get("id"))?;
    let dir = directory(Path::new(get("dipinstallpath")))?;
    let title = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Some(Installed {
        id: id.into(),
        title,
        dir,
        executables: vec![],
        action: Action::Uri(format!(
            "origin2://game/launch/?offerIds={}&autoDownload=false",
            component(id)
        )),
    }))
}
pub(super) fn ea_xml(bytes: &[u8], install: &Path) -> Result<Option<Installed>> {
    let dir = directory(install)?;
    let mut xml = quick_xml::Reader::from_reader(bytes);
    let mut title = String::new();
    let mut id = String::new();
    let mut exe = String::new();
    loop {
        match xml.read_event() {
            Ok(quick_xml::events::Event::Start(e)) => {
                let tag = e.local_name().as_ref().to_ascii_lowercase();
                if ["gametitle", "contentid", "filepath"].contains(&tag.as_str()) {
                    let raw = xml
                        .read_text(e.name())
                        .map_err(|_| Error::msg("Invalid EA installer metadata."))?;
                    let value = quick_xml::escape::unescape(raw.as_ref())
                        .map_err(|_| Error::msg("Invalid EA installer metadata."))?
                        .into_owned();
                    match tag.as_str() {
                        "gametitle" if title.is_empty() => title = value,
                        "contentid" if id.is_empty() => id = value,
                        "filepath" if exe.is_empty() => exe = value,
                        _ => {}
                    }
                }
            }
            Ok(quick_xml::events::Event::DocType(_)) => {
                return Err(Error::msg("EA installer metadata with DTD is unsupported."))
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => return Err(Error::msg("Invalid EA installer metadata.")),
            _ => {}
        }
    }
    if title.is_empty() || id.is_empty() {
        return Err(Error::msg(
            "EA installer metadata has no game title or content ID.",
        ));
    }
    identifier(&id)?;
    if title.len() > 1024 {
        return Err(Error::msg("Invalid EA game title."));
    }
    let relative = if exe.to_ascii_uppercase().starts_with("[INSTALLDIR]") {
        exe[12..].trim_start_matches(['\\', '/'])
    } else {
        exe.as_str()
    };
    let executables = candidate(&dir, relative).into_iter().collect();
    Ok(Some(Installed {
        id: id.clone(),
        title,
        dir,
        executables,
        action: Action::Uri(format!(
            "origin2://game/launch/?offerIds={}&autoDownload=false",
            component(&id)
        )),
    }))
}
fn ea_catalog(root: &Path) -> Result<Catalog> {
    let inventory = inventory("ea")?;
    let mut c = Catalog {
        complete: inventory.complete,
        warnings: inventory.warnings,
        ..Default::default()
    };
    for row in inventory.games {
        let result = (|| {
            let path = crate::steam::inside(&row.dir, "__Installer/installerdata.xml")
                .or_else(|| crate::steam::inside(&row.dir, "_Installer/installerdata.xml"))
                .ok_or_else(|| Error::msg("EA installer metadata unavailable."))?;
            ea_xml(&read(&path)?, &row.dir)
        })();
        collect(&mut c, result);
    }
    let legacy = root.join("Origin/LocalContent");
    if legacy.is_dir() {
        let mut examined = 0;
        for p in entries(&legacy)? {
            let files = if p.is_dir() { entries(&p)? } else { vec![p] };
            examined += files.len();
            if examined > 4096 {
                return Err(Error::msg(
                    "Too many EA manifest entries; previous games were kept.",
                ));
            }
            for file in files {
                if file
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("mfst"))
                {
                    let result = (|| {
                        ea_manifest(
                            std::str::from_utf8(&read(&file)?)
                                .map_err(|_| Error::msg("EA manifest is not UTF-8."))?,
                        )
                    })();
                    collect(&mut c, result);
                }
            }
        }
    }
    let modern = root.join("EA Desktop/InstallData");
    if !modern.is_dir() && !c.complete && !legacy_exists(root) {
        c.warnings
            .push("EA installed-game records are unavailable; previous games were kept.".into());
    }
    if modern.is_dir() && c.games.is_empty() && fs::read_dir(&modern)?.next().is_some() {
        c.complete = false;
        c.warnings.push("EA has install-data folders but no validated installed game records. Previous entries were kept; folder names alone are not imported.".into());
    }
    // Metadata directories alone never prove a game is installed.
    let mut seen = HashSet::new();
    c.games.retain(|g| seen.insert(g.id.clone()));
    Ok(c)
}
fn legacy_exists(root: &Path) -> bool {
    root.join("Origin/LocalContent").is_dir()
}
fn varint(bytes: &[u8], at: &mut usize) -> Result<u64> {
    let mut n = 0u64;
    for i in 0..10 {
        let b = *bytes
            .get(*at)
            .ok_or_else(|| Error::msg("Truncated launcher metadata."))?;
        *at += 1;
        if i == 9 && b > 1 {
            return Err(Error::msg("Invalid launcher metadata integer."));
        }
        n |= u64::from(b & 127) << (i * 7);
        if b & 128 == 0 {
            return Ok(n);
        }
    }
    Err(Error::msg("Invalid launcher metadata integer."))
}
pub(super) fn fields(bytes: &[u8]) -> Result<Vec<(u32, &[u8])>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if out.len() > 4096 {
            return Err(Error::msg("Too many launcher metadata fields."));
        }
        let tag = varint(bytes, &mut at)?;
        let number = u32::try_from(tag >> 3).map_err(|_| Error::msg("Invalid field number."))?;
        if number == 0 {
            return Err(Error::msg("Invalid field number."));
        }
        let size = match tag & 7 {
            0 => {
                varint(bytes, &mut at)?;
                continue;
            }
            1 => 8,
            2 => usize::try_from(varint(bytes, &mut at)?)
                .map_err(|_| Error::msg("Invalid field size."))?,
            5 => 4,
            _ => return Err(Error::msg("Unsupported launcher metadata format.")),
        };
        let end = at
            .checked_add(size)
            .filter(|e| *e <= bytes.len())
            .ok_or_else(|| Error::msg("Truncated launcher metadata."))?;
        if tag & 7 == 2 {
            out.push((number, &bytes[at..end]));
        }
        at = end;
    }
    Ok(out)
}
fn field<'a>(items: &[(u32, &'a [u8])], key: u32) -> Option<&'a [u8]> {
    items.iter().find(|p| p.0 == key).map(|p| p.1)
}
pub(super) fn battle(bytes: &[u8], client: &Path) -> Result<Catalog> {
    let products: Value = serde_json::from_str(include_str!("battlenet-products.json"))?;
    let mut c = Catalog {
        complete: true,
        ..Default::default()
    };
    for (tag, entry) in fields(bytes)? {
        if tag != 1 {
            continue;
        }
        let entry = fields(entry)?;
        let Some(raw) = field(&entry, 1) else {
            continue;
        };
        let internal = std::str::from_utf8(raw)
            .map_err(|_| Error::msg("Invalid Battle.net product ID."))?
            .to_ascii_lowercase();
        if internal == "battle.net" || internal == "agent" {
            continue;
        }
        let Some(product) = products.get(&internal) else {
            c.complete = false;
            c.warnings.push(
                "An unrecognized Battle.net product was skipped; previous games were kept.".into(),
            );
            continue;
        };
        collect(
            &mut c,
            (|| {
                let data = fields(
                    field(&entry, 3)
                        .ok_or_else(|| Error::msg("Missing Battle.net install data."))?,
                )?;
                let path = std::str::from_utf8(
                    field(&data, 1)
                        .ok_or_else(|| Error::msg("Missing Battle.net install path."))?,
                )
                .map_err(|_| Error::msg("Invalid install path."))?;
                let dir = directory(Path::new(path))?;
                let id = identifier(text(product, "code").unwrap_or(""))?;
                let title = text(product, "name").unwrap_or(id).into();
                let names: &[&str] = match internal.as_str() {
                    "wow" => &["_retail_/Wow.exe", "Wow.exe"],
                    "wow_classic_era" => &["_classic_era_/WowClassic.exe"],
                    "wow_classic_anniversary" => &["_anniversary_/WowClassic.exe"],
                    "wow_classic" => &["_classic_/WowClassic.exe"],
                    "wowf" => &["_forever_/Wow.exe"],
                    "w3" => &[
                        "_retail_/x86_64/Warcraft III.exe",
                        "x86_64/Warcraft III.exe",
                    ],
                    "prometheus" | "pro" => &["_retail_/Overwatch.exe", "Overwatch.exe"],
                    "diablo3" | "d3" => &["x64/Diablo III64.exe", "Diablo III.exe"],
                    "fen" | "fenris" => &["Diablo IV.exe"],
                    "osi" => &["D2R.exe"],
                    "hs_beta" | "hsb" => &["Hearthstone.exe"],
                    _ => &[],
                };
                if !client.is_file()
                    || !client
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("Battle.net.exe"))
                {
                    return Err(Error::msg("Battle.net client was not found."));
                }
                Ok(Some(Installed {
                    id: id.into(),
                    title,
                    dir: dir.clone(),
                    executables: names.iter().filter_map(|p| candidate(&dir, p)).collect(),
                    action: Action::Client {
                        exe: fs::canonicalize(client)?,
                        args: vec![format!("--exec=launch {id}")],
                    },
                }))
            })(),
        );
    }
    Ok(c)
}
fn xbox_catalog() -> Result<Catalog> {
    let i = inventory("xboxpc")?;
    let mut c = Catalog {
        complete: i.complete,
        warnings: i.warnings,
        ..Default::default()
    };
    for row in i.games {
        collect(
            &mut c,
            (|| {
                identifier(&row.id)?;
                if row.id.matches('!').count() != 1 {
                    return Err(Error::msg("Invalid Xbox app identity."));
                }
                let dir = directory(&row.dir)?;
                let mut title = row.name;
                let mut executables = row
                    .exes
                    .iter()
                    .filter_map(|p| candidate(&dir, p))
                    .collect::<Vec<_>>();
                let config = dir.join("MicrosoftGame.config");
                if config.is_file() {
                    let bytes = read(&config)?;
                    let mut xml = quick_xml::Reader::from_reader(bytes.as_slice());
                    loop {
                        match xml.read_event() {
                            Ok(
                                quick_xml::events::Event::Start(e)
                                | quick_xml::events::Event::Empty(e),
                            ) => {
                                for a in e.attributes() {
                                    let a = a.map_err(|_| {
                                        Error::msg("Invalid Xbox game configuration.")
                                    })?;
                                    let value = a
                                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                        .map_err(|_| {
                                            Error::msg("Invalid Xbox game configuration.")
                                        })?;
                                    if e.local_name().as_ref() == "Executable"
                                        && a.key.as_ref() == "Name"
                                    {
                                        if let Some(p) = candidate(&dir, &value) {
                                            executables.push(p);
                                        }
                                    }
                                    if e.local_name().as_ref() == "ShellVisuals"
                                        && a.key.as_ref() == "DefaultDisplayName"
                                    {
                                        title = value.into_owned();
                                    }
                                }
                            }
                            Ok(quick_xml::events::Event::Eof) => break,
                            Err(_) => return Err(Error::msg("Invalid Xbox game configuration.")),
                            _ => {}
                        }
                    }
                }
                if title.is_empty() || title.starts_with("ms-resource:") {
                    title = dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                }
                executables.sort();
                executables.dedup();
                Ok(Some(Installed {
                    id: row.id.clone(),
                    title,
                    dir,
                    executables,
                    action: Action::Xbox(row.id),
                }))
            })(),
        );
    }
    Ok(c)
}
pub fn discover(provider: &str) -> Result<Catalog> {
    name(provider)?;
    match provider {
        "epic" => epic_catalog(&data_root()),
        "gog" => gog_catalog(),
        "ubisoft" => ubisoft_catalog(),
        "ea" => ea_catalog(&data_root()),
        "amazon" => amazon_catalog(
            &PathBuf::from(
                std::env::var_os("LOCALAPPDATA")
                    .ok_or_else(|| Error::msg("Local launcher data location is unavailable."))?,
            )
            .join("Amazon Games/Data/Games/Sql/GameInstallInfo.sqlite"),
        ),
        "battlenet" => {
            let i = inventory(provider)?;
            battle(
                &read(&data_root().join("Battle.net/Agent/product.db"))?,
                &i.client
                    .ok_or_else(|| Error::msg("Battle.net client was not found."))?,
            )
        }
        "xboxpc" => xbox_catalog(),
        _ => unreachable!(),
    }
}
