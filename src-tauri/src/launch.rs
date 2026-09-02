//! Starting the engine on a scenario.
//!
//! Splaunch does not join anything, so this is much smaller than the lobby's
//! version it came from: find the install, find the engine, run it on a script
//! we wrote, and come back when it exits.
//!
//! One game at a time. Zero-K is single-instance, and two engines fighting over
//! the same write directory corrupts the config.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::install::{self, Install};

/// Where the engine's comings and goings are announced.
const GAME_EVENT: &str = "splaunch://game";

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GameStatus {
    Launched { pid: u32 },
    Exited { code: Option<i32> },
    Failed { reason: String },
}

pub struct SpawnPlan {
    pub exe: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(String, OsString)>,
}

/// How the engine is invoked: its own config if there is one, then the script.
///
/// The data directory travels in the environment rather than as a flag, because
/// engine versions disagree about the spelling of the write-dir option and an
/// unrecognised flag aborts startup, whereas `SPRING_DATADIR` has been stable
/// for a decade. Without it the engine writes into the user's Documents folder
/// and finds none of the installed games or maps - which looks, from the
/// outside, exactly like a bad start script.
///
/// The working directory is the engine's own folder, not the data root: that is
/// where it looks for the libraries it ships beside itself.
///
/// Both of these are Shiro's, and they are here because Splaunch's own copy had
/// dropped them. A scenario launched without them cannot find Zero-K at all.
pub fn spawn_plan(exe: &Path, root: &Path, script: &Path) -> SpawnPlan {
    let mut args: Vec<OsString> = Vec::new();
    let config = root.join("springsettings.cfg");
    if config.is_file() {
        args.push("--config".into());
        args.push(config.into_os_string());
    }
    args.push(script.as_os_str().to_os_string());
    SpawnPlan {
        exe: exe.to_path_buf(),
        args,
        cwd: exe.parent().unwrap_or(root).to_path_buf(),
        env: vec![
            ("SPRING_DATADIR".into(), root.as_os_str().to_os_string()),
            ("SPRING_WRITEDIR".into(), root.as_os_str().to_os_string()),
        ],
    }
}

/// One game at a time, and where Zero-K lives if it is somewhere unusual.
#[derive(Default)]
pub struct Game {
    running: Arc<Mutex<bool>>,
    root: Arc<Mutex<Option<String>>>,
}

/// What a launch would do, resolved but not run.
///
/// `engine` and `game` are the fields this used to be missing. The frontend
/// read `preview.engine` from the very beginning; nothing ever wrote it, so the
/// editor launched against engine `""` and compiled every scenario with an
/// empty `GameType`. Inside the lobby both arrived from the server, and
/// standing alone nothing replaced them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPreview {
    pub install: Install,
    pub exe: String,
    /// The engine version this would actually run.
    pub engine: String,
    /// The archive name `GameType` would carry.
    pub game: Option<String>,
}

impl Game {
    /// The install this session is using, if there is one.
    ///
    /// Resolved rather than remembered: the override may have been set after
    /// the last detection, and a missing install is a `None` rather than an
    /// error because the callers of this ask questions that are fine to leave
    /// unanswered.
    pub fn install_root(&self) -> Option<PathBuf> {
        let root = self.root.lock().ok().and_then(|r| r.clone());
        install::detect_with(root.as_deref()).ok().map(|i| i.root)
    }
}

/// Find Zero-K, optionally somewhere the user pointed us.
#[tauri::command]
pub fn sp_locate_install(game: State<'_, Game>, root: Option<String>) -> Result<Install, String> {
    {
        let mut r = game.root.lock().map_err(|_| "install state poisoned".to_string())?;
        *r = root.clone().filter(|s| !s.trim().is_empty());
    }
    install::detect_with(root.as_deref())
}

/// Answer "would this work, and with what" without starting anything.
///
/// An empty `engine` means "whichever one is installed" rather than an error:
/// the version is not the author's to choose, and the editor has no way to know
/// it before asking. That is why this resolves it here and hands it back.
#[tauri::command]
pub fn sp_launch_preview(game: State<'_, Game>, engine: String) -> Result<LaunchPreview, String> {
    let root = game.root.lock().ok().and_then(|r| r.clone());
    let install = install::detect_with(root.as_deref())?;
    let chosen = resolve_engine(&install.root, &engine)?;
    let exe = install::find_engine(&install.root, &chosen)?;
    Ok(LaunchPreview {
        exe: exe.display().to_string(),
        engine: chosen,
        game: crate::game::base_game(&install.root).map(|g| g.name),
        install,
    })
}

/// The engine version to actually use.
///
/// A version the caller named is honoured, so a scenario pinned to one keeps
/// running on it. Otherwise the newest installed one wins, and an install with
/// none says so in the sentence rather than failing at the spawn.
fn resolve_engine(root: &Path, asked: &str) -> Result<String, String> {
    let asked = asked.trim();
    if !asked.is_empty() {
        return Ok(asked.to_string());
    }
    crate::game::engine_versions(root).into_iter().next().ok_or_else(|| {
        format!(
            "Zero-K is installed at {} but no engine is. Start a game once in the \
             official lobby to download one, then try again.",
            root.display()
        )
    })
}

/// What the install contains: engines, games, AIs, and the defaults to use.
#[tauri::command]
pub fn sp_game_info(game: State<'_, Game>) -> Result<crate::game::GameInfo, String> {
    let root = game.root.lock().ok().and_then(|r| r.clone());
    let install = install::detect_with(root.as_deref())?;
    Ok(crate::game::game_info(&install.root))
}

/// The placeable roster.
///
/// Read out of the installed game where that is possible, and from the vendored
/// copy otherwise, because an editor with no unit list is not usable and a
/// machine without Zero-K should still be able to build a scenario to run
/// elsewhere. Which one answered is reported rather than hidden.
#[tauri::command]
pub fn sp_units(game: State<'_, Game>) -> crate::game::Roster {
    let root = game.root.lock().ok().and_then(|r| r.clone());
    let installed = install::detect_with(root.as_deref())
        .ok()
        .and_then(|i| crate::game::base_game(&i.root))
        .and_then(|archive| crate::game::read_unit_defs(&archive.path).ok())
        .filter(|units| !units.is_empty());

    match installed {
        Some(units) => crate::game::Roster { source: "the installed game".into(), units },
        None => crate::game::Roster {
            source: format!("Splaunch's vendored copy of Zero-K ({})", &crate::game::ROSTER_PIN[..7]),
            units: crate::game::vendored_units(),
        },
    }
}

/// Pictures for the named units, as `data:` URIs.
///
/// Empty rather than an error when there is no install: the editor draws the
/// plain markers it always drew, and a machine without Zero-K can still author
/// a scenario.
#[tauri::command]
pub fn sp_unit_icons(
    game: State<'_, Game>,
    names: Vec<String>,
) -> std::collections::HashMap<String, String> {
    let Some(root) = game.install_root() else { return Default::default() };
    let Some(archive) = crate::game::base_game(&root) else { return Default::default() };
    let defs = crate::game::read_unit_defs(&archive.path).unwrap_or_default();
    crate::game::unit_icons(&archive.path, &defs, &names)
}

/// Zoom-out icons for the named units, decoded to RGBA.
///
/// Separate from `sp_unit_icons`, which returns build pictures: these are the
/// flat silhouettes the game draws when you zoom out, and they are what stays
/// readable at the size a map marker actually is.
#[tauri::command]
pub fn sp_unit_marks(
    game: State<'_, Game>,
    names: Vec<String>,
) -> std::collections::HashMap<String, crate::game::Mark> {
    let Some(root) = game.install_root() else { return Default::default() };
    let Some(archive) = crate::game::base_game(&root) else { return Default::default() };
    let defs = crate::game::read_unit_defs(&archive.path).unwrap_or_default();
    crate::game::unit_marks(&archive.path, &defs, &names)
}

/// The metal infomap for a map, if its archive can be found and read.
///
/// `None` rather than an error throughout: a map that is not installed, or one
/// whose archive this cannot open, means the editor draws no metal - which is
/// what it did before - not that the scenario is broken.
/// `command(async)` because reading it is not cheap: a `.sd7` is one solid
/// LZMA block, so reaching the `.smf` means inflating whatever sits before it -
/// usually the tile file, which runs to hundreds of megabytes. On the main
/// thread that is the webview frozen for seconds every time the map changes.
#[tauri::command(async)]
pub fn sp_map_metal(game: State<'_, Game>, map: String) -> Option<crate::mapfile::MetalMap> {
    let root = game.install_root()?;
    let archive = crate::mapfile::map_archive(&root, &map)?;
    crate::mapfile::metal_for(&archive)
}

/// Start the engine on a script somebody else wrote.
///
/// `running` is cleared however it ends, and the exit is announced, so the
/// editor comes back by itself when the match is over.
pub fn launch_script(
    app: AppHandle,
    game: State<'_, Game>,
    script: &Path,
    engine: &str,
) -> Result<u32, String> {
    {
        let mut running = game
            .running
            .lock()
            .map_err(|_| "game state poisoned".to_string())?;
        if *running {
            return Err("A game is already running.".into());
        }
        *running = true;
    }

    let root = game.root.lock().ok().and_then(|r| r.clone());
    let spawned = (|| {
        let install = install::detect_with(root.as_deref())?;
        let exe = install::find_engine(&install.root, engine)?;
        let plan = spawn_plan(&exe, &install.root, script);
        let mut cmd = std::process::Command::new(&plan.exe);
        cmd.args(&plan.args).current_dir(&plan.cwd);
        for (k, v) in &plan.env {
            cmd.env(k, v);
        }
        cmd.spawn()
            .map_err(|e| format!("could not start the engine: {e}"))
    })();

    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            if let Ok(mut r) = game.running.lock() {
                *r = false;
            }
            let _ = app.emit(GAME_EVENT, GameStatus::Failed { reason: e.clone() });
            return Err(e);
        }
    };

    let pid = child.id();
    let _ = app.emit(GAME_EVENT, GameStatus::Launched { pid });

    let running = game.running.clone();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        if let Ok(mut r) = running.lock() {
            *r = false;
        }
        let _ = app.emit(GAME_EVENT, GameStatus::Exited { code });
    });

    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_gets_the_config_when_there_is_one() {
        // The engine reads springsettings.cfg from the data directory, and a
        // scenario should look the way the player's game looks.
        let dir = std::env::temp_dir().join("splaunch-test-plan");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("springsettings.cfg"), "x=1").unwrap();

        // --config, the path to it, then the script - in that order, because
        // the engine takes the script as its positional argument.
        let plan = spawn_plan(Path::new("spring.exe"), &dir, Path::new("script.txt"));
        assert_eq!(plan.args.len(), 3);
        assert_eq!(plan.args[0], OsString::from("--config"));
        assert_eq!(plan.args[2], OsString::from("script.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_config_it_is_just_the_script() {
        let dir = std::env::temp_dir().join("splaunch-test-plan-bare");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let plan = spawn_plan(Path::new("spring.exe"), &dir, Path::new("script.txt"));
        assert_eq!(plan.args, vec![OsString::from("script.txt")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_engine_is_told_where_zero_k_lives() {
        /* Splaunch used to spawn with no environment at all, which is the
           quietest possible way to fail: the engine starts, writes into
           Documents, finds no games or maps, and the scenario looks like a bad
           script. Shiro has always passed these two. */
        let plan = spawn_plan(
            Path::new("/zk/engine/linux64/2025.06.21/spring"),
            Path::new("/zk"),
            Path::new("script.txt"),
        );
        let data = plan.env.iter().find(|(k, _)| k == "SPRING_DATADIR");
        assert_eq!(data.map(|(_, v)| v.as_os_str()), Some(Path::new("/zk").as_os_str()));
        let write = plan.env.iter().find(|(k, _)| k == "SPRING_WRITEDIR");
        assert_eq!(write.map(|(_, v)| v.as_os_str()), Some(Path::new("/zk").as_os_str()));
    }

    #[test]
    fn the_engine_runs_from_its_own_folder() {
        // Where the libraries it ships beside itself are.
        let plan = spawn_plan(
            Path::new("/zk/engine/linux64/2025.06.21/spring"),
            Path::new("/zk"),
            Path::new("script.txt"),
        );
        assert_eq!(plan.cwd, Path::new("/zk/engine/linux64/2025.06.21"));
    }
}
