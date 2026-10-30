//! Launch-template rendering.
//!
//! A template such as `"{emulator}" -L "{core}" -f "{rom}"` is first split into
//! argv tokens (double quotes group, backslashes are literal so Windows paths
//! work), and only then are placeholders substituted inside each token. A path
//! containing spaces, quotes or `&` therefore always stays one argument, and no
//! shell is ever involved.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Values available to a template. `core` is `None` for standalone emulators.
#[derive(Debug, Clone)]
pub struct Context {
    pub emulator: PathBuf,
    pub core: Option<PathBuf>,
    pub rom: PathBuf,
}

pub fn tokenize(template: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has_token = false; // distinguishes `""` (empty arg) from nothing

    for c in template.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if in_quotes {
        return Err(Error::Template("unterminated quote in launch template".into()));
    }
    if has_token {
        tokens.push(cur);
    }
    if tokens.is_empty() {
        return Err(Error::Template("launch template is empty".into()));
    }
    Ok(tokens)
}

/// Resolves a core name to a libretro DLL path. A value that already looks like a
/// path (has a separator or ends in .dll) is used as-is.
pub fn resolve_core(cores_dir: Option<&Path>, core: &str) -> Result<PathBuf> {
    let looks_like_path = core.contains(['/', '\\']) || core.to_ascii_lowercase().ends_with(".dll");
    if looks_like_path {
        return Ok(PathBuf::from(core));
    }
    let dir = cores_dir
        .ok_or_else(|| Error::Template("RetroArch cores folder is not set in Settings".into()))?;
    Ok(dir.join(format!("{core}_libretro.dll")))
}

fn lookup(name: &str, ctx: &Context) -> Result<String> {
    let path_str = |p: &Path| p.to_string_lossy().into_owned();
    Ok(match name {
        "emulator" => path_str(&ctx.emulator),
        "core" => path_str(
            ctx.core
                .as_deref()
                .ok_or_else(|| Error::Template("template uses {core} but this platform has no core".into()))?,
        ),
        "rom" => path_str(&ctx.rom),
        "rom_dir" => ctx.rom.parent().map(path_str).unwrap_or_default(),
        "rom_name" => ctx
            .rom
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        other => return Err(Error::Template(format!("unknown placeholder {{{other}}}"))),
    })
}

fn substitute(token: &str, ctx: &Context) -> Result<String> {
    let mut out = String::with_capacity(token.len());
    let mut rest = token;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                out.push_str(&lookup(&after[..end], ctx)?);
                rest = &after[end + 1..];
            }
            None => {
                return Err(Error::Template(format!("unclosed '{{' in template token `{token}`")))
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Renders a template into argv. `argv[0]` is the program to run.
pub fn render(template: &str, ctx: &Context) -> Result<Vec<String>> {
    tokenize(template)?.iter().map(|t| substitute(t, ctx)).collect()
}

/// Checks a template without real paths, for validation in Settings.
pub fn validate(template: &str, needs_core: bool) -> Result<()> {
    let tokens = tokenize(template)?;
    let joined = tokens.join(" ");
    if !joined.contains("{rom}") && !joined.contains("{rom_dir}") {
        return Err(Error::Template("template must contain {rom} (or {rom_dir})".into()));
    }
    let ctx = Context {
        emulator: "emu.exe".into(),
        core: needs_core.then(|| PathBuf::from("core.dll")),
        rom: "C:/game.bin".into(),
    };
    tokens.iter().try_for_each(|t| substitute(t, &ctx).map(drop))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(rom: &str) -> Context {
        Context {
            emulator: r"E:\Emulators\RetroArch\retroarch.exe".into(),
            core: Some(r"E:\Emulators\RetroArch\cores\mgba_libretro.dll".into()),
            rom: rom.into(),
        }
    }

    #[test]
    fn tokenizes_quotes_and_keeps_backslashes() {
        let t = tokenize(r#""C:\Program Files\x.exe" -a "b c" d"#).unwrap();
        assert_eq!(t, [r"C:\Program Files\x.exe", "-a", "b c", "d"]);
    }

    #[test]
    fn empty_quoted_token_is_an_argument() {
        assert_eq!(tokenize(r#"a "" b"#).unwrap(), ["a", "", "b"]);
    }

    #[test]
    fn rejects_unterminated_quote_and_empty() {
        assert!(tokenize(r#""{emulator} -f"#).is_err());
        assert!(tokenize("   ").is_err());
    }

    #[test]
    fn renders_retroarch_template() {
        let argv = render(
            r#""{emulator}" -L "{core}" -f "{rom}""#,
            &ctx(r"E:\Emulator Games\Nintendo\GBA\Golden Sun (USA, Europe).gba"),
        )
        .unwrap();
        assert_eq!(
            argv,
            [
                r"E:\Emulators\RetroArch\retroarch.exe",
                "-L",
                r"E:\Emulators\RetroArch\cores\mgba_libretro.dll",
                "-f",
                r"E:\Emulator Games\Nintendo\GBA\Golden Sun (USA, Europe).gba",
            ]
        );
    }

    #[test]
    fn paths_with_ampersand_quotes_and_spaces_stay_one_argument() {
        let rom = r#"E:\Emulator Games\PS3\Kane & Lynch 2 - Dog Days (USA) (En,Fr,Es)\PS3_GAME\USRDIR\EBOOT.BIN"#;
        let argv = render(r#""{emulator}" --no-gui --fullscreen "{rom}""#, &ctx(rom)).unwrap();
        assert_eq!(argv.len(), 4);
        assert_eq!(argv[3], rom);

        // Even unquoted in the template, a substituted path is never re-split.
        let argv = render("{emulator} {rom}", &ctx(r#"D:\a b\"odd" name.iso"#)).unwrap();
        assert_eq!(argv, [r"E:\Emulators\RetroArch\retroarch.exe", r#"D:\a b\"odd" name.iso"#]);
    }

    #[test]
    fn dolphin_config_args_and_rom_dir_name() {
        let argv = render(
            r#""{emulator}" -b -C Dolphin.Display.Fullscreen=True -e "{rom}" --x="{rom_dir}/{rom_name}.sav""#,
            &ctx(r"G:\GC\Luigi's Mansion (USA).iso"),
        )
        .unwrap();
        assert_eq!(argv[2], "-C");
        assert_eq!(argv[3], "Dolphin.Display.Fullscreen=True");
        assert_eq!(argv[5], r"G:\GC\Luigi's Mansion (USA).iso");
        assert_eq!(argv[6], r"--x=G:\GC/Luigi's Mansion (USA).sav");
    }

    #[test]
    fn core_placeholder_without_core_is_an_error() {
        let mut c = ctx("x.iso");
        c.core = None;
        assert!(render(r#""{emulator}" -L "{core}" "{rom}""#, &c).is_err());
    }

    #[test]
    fn unknown_or_unclosed_placeholder_is_an_error() {
        assert!(render("{emulator} {bogus}", &ctx("x")).is_err());
        assert!(render("{emulator} {rom", &ctx("x")).is_err());
    }

    #[test]
    fn resolves_core_names_and_paths() {
        let dir = Path::new(r"E:\Emulators\RetroArch\cores");
        assert_eq!(
            resolve_core(Some(dir), "mgba").unwrap(),
            dir.join("mgba_libretro.dll")
        );
        assert_eq!(
            resolve_core(None, r"D:\cores\custom.dll").unwrap(),
            PathBuf::from(r"D:\cores\custom.dll")
        );
        assert!(resolve_core(None, "mgba").is_err());
    }

    #[test]
    fn validate_requires_rom() {
        assert!(validate(r#""{emulator}" -f"#, false).is_err());
        assert!(validate(r#""{emulator}" -f "{rom}""#, false).is_ok());
        assert!(validate(r#""{emulator}" -L "{core}" "{rom}""#, false).is_err());
        assert!(validate(r#""{emulator}" -L "{core}" "{rom}""#, true).is_ok());
    }
}
