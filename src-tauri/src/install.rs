//! Locating an existing Zero-K installation.
//!
//! Splaunch never downloads content: it reuses the engine, games and maps of
//! whatever Zero-K install is already on the machine.
//! That makes this module the first thing standing between an author and a
//! launch, and `game.rs` the second - this finds the install, that reads what is
//! inside it.
//!
//! Everything here is deliberately pure except `detect()` itself, so the path
//! logic can be tested without a Zero-K install present.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A Zero-K data directory - the folder holding `engine/`, `games/`, `maps/`.
/// Spring calls this the write dir; ZK keeps everything under it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub root: PathBuf,
    /// How we found it, so the UI can say "Steam" rather than a bare path.
    pub source: String,
}

/// Directories that make a folder recognisably a Zero-K data dir rather than
/// some unrelated folder that happens to be called Zero-K. `engine` alone is
/// not enough - a bare engine checkout has one too.
fn looks_like_zk_root(root: &Path) -> bool {
    root.join("engine").is_dir()
        && (root.join("games").is_dir() || root.join("pool").is_dir() || root.join("packages").is_dir())
}

/// Pull library paths out of Steam's `libraryfolders.vdf`.
///
/// The file is Valve's KeyValues format. We want exactly one thing from it -
/// the `"path"` of each library - so we scan for that key instead of pulling in
/// a VDF parser. Both the flat (pre-2021) and nested layouts put the value on
/// the same line as the key, which is why this holds up.
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in vdf.lines() {
        let mut parts = line.split('"').skip(1);
        let Some(key) = parts.next() else { continue };
        if key != "path" {
            continue;
        }
        // skip the whitespace between key and value
        let Some(value) = parts.nth(1) else { continue };
        if value.is_empty() {
            continue;
        }
        // VDF escapes backslashes, so "D:\\Games" is the path D:\Games.
        out.push(PathBuf::from(value.replace("\\\\", "\\")));
    }
    out
}

/// Steam roots to probe when `libraryfolders.vdf` has not been found yet.
fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Ok(dir) = std::env::var(var) {
                roots.push(PathBuf::from(dir).join("Steam"));
            }
        }
    }
    if let Some(home) = home_dir() {
        roots.push(home.join(".local/share/Steam"));
        roots.push(home.join(".steam/steam"));
        roots.push(home.join("Library/Application Support/Steam"));
    }
    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Every place a Zero-K data dir plausibly lives, most specific first.
/// Returned in probe order; the first hit that `looks_like_zk_root` wins.
fn candidates() -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();

    /* Shiro hands us the directory it manages, in `SHIRO_ZK_ROOT`, when it is
       the one starting us. It is first because it is the only candidate that
       was told to us rather than guessed at.

       Without this the managed install was invisible: it lives under
       `%APPDATA%\info.zero-k.shiro\zk`, which none of the guesses below look
       at, so Splaunch launched from Shiro's own app list could not find the
       only Zero-K on the machine. */
    if let Ok(root) = std::env::var("SHIRO_ZK_ROOT") {
        let root = root.trim();
        if !root.is_empty() {
            out.push((PathBuf::from(root), "Shiro".into()));
        }
    }

    /* And the same directory by its own path, for a launch that did not come
       through Shiro - a desktop shortcut, or the app started by hand. The
       identifier is Shiro's bundle id; it is a literal here because Splaunch
       does not otherwise know anything about Shiro. */
    if let Ok(roaming) = std::env::var("APPDATA") {
        out.push((
            PathBuf::from(&roaming).join("info.zero-k.shiro").join("zk"),
            "Shiro's managed install".into(),
        ));
    }

    // The standalone installer's default, and where our own NSIS package goes.
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push((
            PathBuf::from(&local).join("Programs").join("Zero-K"),
            "Zero-K installer".into(),
        ));
    }
    if let Some(home) = home_dir() {
        out.push((home.join("Zero-K"), "home directory".into()));
        // Linux ZK installs write into the Spring data dir.
        out.push((home.join(".config/spring"), "Spring data directory".into()));
        out.push((home.join(".spring"), "Spring data directory".into()));
    }

    // Steam: read every library, not just the default one. People move games
    // to a second drive constantly and then wonder why a lobby cannot find them.
    for steam in steam_roots() {
        let mut libraries = vec![steam.clone()];
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            libraries.extend(parse_library_folders(&text));
        }
        for lib in libraries {
            out.push((
                lib.join("steamapps").join("common").join("Zero-K"),
                "Steam".into(),
            ));
        }
    }
    out
}

/// Find the Zero-K install, or explain where we looked.
///
/// `override_root` is a path someone typed into settings. It is checked like
/// any other candidate rather than trusted, so a typo says "that is not a
/// Zero-K folder" instead of failing later at the engine.
pub fn detect_with(override_root: Option<&str>) -> Result<Install, String> {
    if let Some(root) = override_root.map(str::trim).filter(|r| !r.is_empty()) {
        let path = PathBuf::from(root);
        if looks_like_zk_root(&path) {
            return Ok(Install { root: path, source: "settings".into() });
        }
        return Err(format!(
            "{} is not a Zero-K installation - no engine/ with games, maps or pool beside it.",
            path.display()
        ));
    }
    detect()
}

/// Find the Zero-K install, or explain where we looked.
pub fn detect() -> Result<Install, String> {
    let probed = candidates();
    for (root, source) in &probed {
        if looks_like_zk_root(root) {
            return Ok(Install {
                root: root.clone(),
                source: source.clone(),
            });
        }
    }
    Err(format!(
        "No Zero-K installation found. Looked in:\n{}",
        probed
            .iter()
            .map(|(p, _)| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

/// Platform subfolder ZK files engines under.
fn engine_platform() -> &'static str {
    if cfg!(windows) {
        "win64"
    } else if cfg!(target_os = "macos") {
        "osx64"
    } else {
        "linux64"
    }
}

fn engine_exe() -> &'static str {
    if cfg!(windows) {
        "spring.exe"
    } else {
        "spring"
    }
}

/// Where the engine binary for `version` should be.
///
/// The server sends `Welcome.Engine` / `ConnectSpring.Engine` as a bare version
/// string ("2025.06.21") and ZK names the directory with exactly that string,
/// so no normalisation is needed. Layout has moved around across ZK versions,
/// hence the fallbacks.
pub fn engine_candidates(root: &Path, version: &str) -> Vec<PathBuf> {
    let exe = engine_exe();
    vec![
        root.join("engine").join(engine_platform()).join(version).join(exe),
        root.join("engine").join(version).join(exe),
        root.join("engine").join(engine_platform()).join(version).join("bin").join(exe),
    ]
}

/// Resolve the engine binary, or say which version is missing.
pub fn find_engine(root: &Path, version: &str) -> Result<PathBuf, String> {
    for path in engine_candidates(root, version) {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(format!(
        "Zero-K is installed at {} but engine {} is not. Start that game once in the \
         official lobby to download the engine, then try again.",
        root.display(),
        version
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_nested_libraryfolders_layout() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}
"#;
        let got = parse_library_folders(vdf);
        assert_eq!(
            got,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn ignores_keys_that_merely_contain_a_path() {
        // "mounted" and "contentstatsid" sit beside "path" in real files.
        let vdf = "\t\"contentstatsid\"\t\t\"12345\"\n\t\"path\"\t\t\"/home/u/.local/share/Steam\"\n";
        assert_eq!(
            parse_library_folders(vdf),
            vec![PathBuf::from("/home/u/.local/share/Steam")]
        );
    }

    #[test]
    fn survives_a_truncated_or_empty_vdf() {
        assert!(parse_library_folders("").is_empty());
        assert!(parse_library_folders("\"path\"").is_empty());
        assert!(parse_library_folders("\"path\" \"\"").is_empty());
    }

    #[test]
    fn an_override_is_checked_like_any_other_candidate() {
        let err = detect_with(Some("/definitely/not/zero-k")).unwrap_err();
        assert!(err.contains("not a Zero-K installation"), "{err}");
    }

    #[test]
    fn a_blank_override_falls_through_to_detection() {
        // Whatever detection returns here, it must not be the override error.
        if let Err(e) = detect_with(Some("   ")) {
            assert!(e.contains("No Zero-K installation found"), "{e}");
        }
    }

    #[test]
    fn engine_lookup_reports_the_version_rather_than_a_bare_not_found() {
        let err = find_engine(Path::new("/nonexistent/Zero-K"), "2025.06.21").unwrap_err();
        assert!(err.contains("2025.06.21"), "{err}");
    }

    #[test]
    fn engine_candidates_lead_with_the_platform_layout() {
        let paths = engine_candidates(Path::new("/zk"), "2025.06.21");
        assert!(paths[0].to_string_lossy().contains(engine_platform()));
        assert!(paths[0].ends_with(engine_exe()));
    }

    #[test]
    fn a_folder_needs_content_beside_the_engine_to_count() {
        let tmp = std::env::temp_dir().join("shiro-install-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("engine")).unwrap();
        assert!(!looks_like_zk_root(&tmp), "engine alone must not qualify");
        std::fs::create_dir_all(tmp.join("pool")).unwrap();
        assert!(looks_like_zk_root(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /* Shiro launches Splaunch with SHIRO_ZK_ROOT set, and its managed install
       lives somewhere none of the other guesses look. Both were missing, so
       Splaunch started from Shiro's own app list could not see the only
       Zero-K on the machine.

       These three share a lock because the thing under test is a process-wide
       environment variable and the test runner is threaded: without it they
       clear each other's setup and fail in a way that depends on scheduling. */
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run `f` with each named variable set to its value, or unset when `None`.
    ///
    /// Every variable at once rather than one per call: the lock is a plain
    /// `Mutex`, so nesting two of these on one thread deadlocks rather than
    /// failing, and the test that needs two would hang instead of erroring.
    fn with_vars<T>(vars: &[(&str, Option<&str>)], f: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect();
        // SAFETY: the lock above makes this the only thread touching them.
        unsafe {
            for (name, value) in vars {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
        let out = f();
        unsafe {
            for (name, value) in &before {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
        out
    }

    /// Run `f` with SHIRO_ZK_ROOT set to `value`, or unset when `None`.
    fn with_handed_root<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        with_vars(&[("SHIRO_ZK_ROOT", value)], f)
    }

    #[test]
    fn the_handed_root_is_probed_before_anything_guessed() {
        let handed = r"C:\handed\zk";
        let c = with_handed_root(Some(handed), candidates);
        assert_eq!(c[0].0, PathBuf::from(handed));
        assert_eq!(c[0].1, "Shiro");
    }

    #[test]
    fn an_empty_handed_root_is_not_a_candidate() {
        let c = with_handed_root(Some("   "), candidates);
        assert!(c.iter().all(|(_, via)| via != "Shiro"),
            "a blank variable must not become a candidate");
    }

    #[test]
    fn the_managed_install_is_probed_even_without_the_variable() {
        /* APPDATA is set here rather than read, because it is Windows-only and
           the thing under test is how `candidates` uses it, not whether the
           machine running the tests happens to have one. Without this the test
           passed on the Windows CI and failed on the Linux release workflow,
           which also runs `cargo test` - so the Linux packages stopped being
           published for a reason that had nothing to do with Linux. */
        let c = with_vars(
            &[
                ("SHIRO_ZK_ROOT", None),
                ("APPDATA", Some(r"C:\Users\test\AppData\Roaming")),
            ],
            candidates,
        );
        assert!(c.iter().any(|(p, _)| p.ends_with("zk")
            && p.to_string_lossy().contains("info.zero-k.shiro")),
            "Shiro's managed directory should be looked at, not only handed over");
    }
}
