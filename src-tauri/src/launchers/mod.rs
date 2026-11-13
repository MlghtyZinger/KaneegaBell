//! Installed-only PC launcher integration. No account APIs or client database writes.
mod readers;
use crate::{
    db::queries,
    error::{Error, Result},
    scanner::{normalize, path_key},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const PROVIDERS: &[(&str, &str)] = &[
    ("epic", "Epic Games"),
    ("gog", "GOG Galaxy"),
    ("battlenet", "Battle.net"),
    ("xboxpc", "Xbox PC"),
    ("amazon", "Amazon Games"),
    ("ubisoft", "Ubisoft Connect"),
    ("ea", "EA"),
];
pub fn name(id: &str) -> Result<&'static str> {
    PROVIDERS
        .iter()
        .find(|p| p.0 == id)
        .map(|p| p.1)
        .ok_or_else(|| Error::msg("Unknown launcher."))
}
pub fn is_launcher(id: &str) -> bool {
    id == "steam" || PROVIDERS.iter().any(|p| p.0 == id)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub provider: String,
    pub product_id: String,
    pub install_dir: PathBuf,
    pub executables: Vec<PathBuf>,
    pub tracking_exe: Option<PathBuf>,
    pub action: Action,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum Action {
    Uri(String),
    Client { exe: PathBuf, args: Vec<String> },
    Xbox(String),
}
impl Action {
    pub fn preview(&self) -> Vec<String> {
        match self {
            Self::Uri(u) => vec![u.clone()],
            Self::Xbox(id) => vec![format!("shell:AppsFolder\\{id}")],
            Self::Client { exe, args } => std::iter::once(exe.to_string_lossy().into_owned())
                .chain(args.clone())
                .collect(),
        }
    }
    pub fn open(&self) -> Result<()> {
        match self {
            Self::Uri(u) => crate::launcher::steam_process::open_uri(u),
            Self::Xbox(id) => crate::launcher::steam_process::activate_xbox(id),
            Self::Client { exe, args } => {
                if !exe.is_file() {
                    return Err(Error::msg(
                        "The launcher executable is missing. Reinstall or repair its client.",
                    ));
                }
                let mut command = std::process::Command::new(exe);
                command
                    .args(args)
                    .current_dir(exe.parent().unwrap_or(Path::new(".")));
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                // Observe the real game separately; never attach the client to a Job Object.
                command
                    .spawn()
                    .map_err(|_| Error::msg("The launcher could not be started."))?;
                Ok(())
            }
        }
    }
}
#[derive(Debug, Clone)]
pub struct Installed {
    pub id: String,
    pub title: String,
    pub dir: PathBuf,
    pub executables: Vec<PathBuf>,
    pub action: Action,
}
#[derive(Default)]
pub struct Catalog {
    pub games: Vec<Installed>,
    pub warnings: Vec<String>,
    pub complete: bool,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
    pub cancelled: bool,
    pub warnings: Vec<String>,
    pub changes: Vec<queries::GameChange>,
}

pub fn connected(conn: &Connection) -> Result<Vec<String>> {
    Ok(
        queries::get_setting::<Vec<String>>(conn, "launchersConnected")?
            .unwrap_or_default()
            .into_iter()
            .filter(|p| name(p).is_ok())
            .collect(),
    )
}
pub fn connect(conn: &mut Connection, provider: &str, enabled: bool) -> Result<()> {
    name(provider)?;
    let mut sources = connected(conn)?;
    sources.retain(|p| p != provider);
    if enabled {
        sources.push(provider.into());
    }
    let tx = conn.transaction()?;
    queries::set_setting(&tx, "launchersConnected", &serde_json::to_value(sources)?)?;
    tx.execute(
        "UPDATE platforms SET enabled=?2 WHERE id=?1",
        params![provider, enabled],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn game(conn: &Connection, id: i64) -> Result<Option<Game>> {
    let row:Option<(String,String,String,String,Option<String>,String)>=conn.query_row("SELECT provider,product_id,install_dir,executables,tracking_exe,action FROM launcher_games WHERE game_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    row.map(|(provider, product_id, dir, exes, chosen, action)| {
        Ok(Game {
            provider,
            product_id,
            install_dir: dir.into(),
            executables: serde_json::from_str(&exes)?,
            tracking_exe: chosen.map(Into::into),
            action: serde_json::from_str(&action)?,
        })
    })
    .transpose()
}
pub fn set_tracking(conn: &Connection, id: i64, path: Option<&Path>) -> Result<()> {
    let g = game(conn, id)?.ok_or_else(|| Error::msg("This is not a launcher game."))?;
    let checked = path
        .map(|p| crate::steam::tracking_file(&g.install_dir, p))
        .transpose()?;
    conn.execute(
        "UPDATE launcher_games SET tracking_exe=?2 WHERE game_id=?1",
        params![id, checked.map(|p| p.to_string_lossy().into_owned())],
    )?;
    Ok(())
}
pub fn validate(g: &Game) -> Result<Installed> {
    let catalog = readers::discover(&g.provider)?;
    let actual = catalog
        .games
        .into_iter()
        .find(|a| a.id == g.product_id)
        .ok_or_else(|| {
            Error::msg("This game is no longer confirmed installed. Scan its launcher again.")
        })?;
    if path_key(&actual.dir) != path_key(&g.install_dir) {
        return Err(Error::msg(
            "This game moved. Scan its launcher before playing.",
        ));
    }
    Ok(actual)
}
fn upsert(conn: &Connection, provider: &str, app: &Installed) -> Result<bool> {
    let launch = format!("kb-launcher:{provider}:{}", component(&app.id));
    let old: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM launcher_games WHERE provider=?1 AND product_id=?2)",
        params![provider, app.id],
        |r| r.get(0),
    )?;
    conn.execute("INSERT INTO games(platform_id,title,sort_title,search_key,launch_path,added_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(launch_path) DO UPDATE SET title=excluded.title,sort_title=excluded.sort_title,search_key=excluded.search_key,missing=0",params![provider,app.title,normalize::sort_title(&app.title),normalize::search_key(&app.title),launch,queries::now()])?;
    let id: i64 = conn.query_row("SELECT id FROM games WHERE launch_path=?1", [launch], |r| {
        r.get(0)
    })?;
    conn.execute("INSERT INTO launcher_games(game_id,provider,product_id,install_dir,executables,action) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(provider,product_id) DO UPDATE SET install_dir=excluded.install_dir,executables=excluded.executables,action=excluded.action,tracking_exe=CASE WHEN launcher_games.install_dir=excluded.install_dir THEN launcher_games.tracking_exe ELSE NULL END",params![id,provider,app.id,app.dir.to_string_lossy(),serde_json::to_string(&app.executables)?,serde_json::to_string(&app.action)?])?;
    Ok(!old)
}
fn import(
    conn: &mut Connection,
    provider: &str,
    catalog: Catalog,
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize, &str),
) -> Result<Summary> {
    let mut out = Summary {
        warnings: catalog.warnings,
        ..Default::default()
    };
    let tx = conn.transaction()?;
    let mut seen = HashSet::new();
    let total = catalog.games.len();
    for (done, app) in catalog.games.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Ok(Summary {
                cancelled: true,
                ..Default::default()
            });
        }
        progress(done, total, &app.title);
        if !seen.insert(app.id.clone()) {
            out.warnings
                .push("Duplicate launcher product ID; previous games kept.".into());
            continue;
        }
        if upsert(&tx, provider, app)? {
            out.added += 1;
            out.changes.push(queries::GameChange {
                kind: "added",
                platform_id: provider.into(),
                from_platform: None,
                title: app.title.clone(),
                variant: None,
            });
        } else {
            out.updated += 1;
        }
    }
    if catalog.complete && out.warnings.is_empty() {
        let mut statement=tx.prepare("SELECT s.game_id,s.product_id,COALESCE(g.user_title,g.title) FROM launcher_games s JOIN games g ON g.id=s.game_id WHERE s.provider=?1 AND g.missing=0 AND g.running_since IS NULL")?;
        let rows = statement
            .query_map([provider], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, product, title) in rows {
            if !seen.contains(&product) {
                tx.execute("UPDATE games SET missing=1 WHERE id=?1", [id])?;
                out.missing += 1;
                out.changes.push(queries::GameChange {
                    kind: "hidden",
                    platform_id: provider.into(),
                    from_platform: None,
                    title,
                    variant: None,
                });
            }
        }
    }
    if cancel.load(Ordering::Acquire) {
        return Ok(Summary {
            cancelled: true,
            ..Default::default()
        });
    }
    tx.commit()?;
    progress(total, total, "Finished");
    queries::set_setting(
        conn,
        &format!("launcherScan.{provider}"),
        &serde_json::to_value(&out)?,
    )?;
    Ok(out)
}
pub fn scan(
    conn: &mut Connection,
    provider: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(usize, usize, &str),
) -> Result<Summary> {
    name(provider)?;
    progress(0, 0, "Reading installed-game records");
    let catalog = readers::discover(provider)?;
    import(conn, provider, catalog, cancel, progress)
}
fn component(id: &str) -> String {
    percent_encoding::utf8_percent_encode(id, percent_encoding::NON_ALPHANUMERIC).to_string()
}
fn identifier(s: &str) -> Result<&str> {
    if s.is_empty()
        || s.len() > 512
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:!".contains(&b))
    {
        Err(Error::msg("Invalid launcher game identifier."))
    } else {
        Ok(s)
    }
}
fn candidate(dir: &Path, relative: &str) -> Option<PathBuf> {
    crate::steam::inside(dir, relative).and_then(|p| crate::steam::tracking_file(dir, &p).ok())
}
fn read(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let f = fs::File::open(path)?;
    if f.metadata()?.len() > 8 * 1024 * 1024 {
        return Err(Error::msg("Launcher metadata is too large."));
    }
    let mut b = Vec::new();
    f.take(8 * 1024 * 1024 + 1).read_to_end(&mut b)?;
    if b.len() > 8 * 1024 * 1024 {
        return Err(Error::msg("Launcher metadata is too large."));
    }
    Ok(b)
}
fn json(path: &Path) -> Result<serde_json::Value> {
    Ok(serde_json::from_slice(&read(path)?)?)
}

#[cfg(test)]
mod tests;
