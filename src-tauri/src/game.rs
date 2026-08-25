//! What the installed Zero-K actually contains.
//!
//! Splaunch used to guess at all of this. The engine version and the game
//! archive name were never discovered at all - inside the lobby they arrived
//! from the server (`Welcome.Engine`, `ConnectSpring.Engine`), and standing
//! alone nothing replaced them, so every scenario compiled with an empty
//! `GameType` and launched against engine `""`. The unit palette was a
//! hand-written list of *Balanced Annihilation* names (`armpw`, `corhlt`)
//! that Zero-K does not define, so every unit placed with it would have
//! spawned nothing.
//!
//! All four answers are on disk already, so this module reads them:
//!
//! - **Engine versions** are directory names under `engine/<platform>/`.
//! - **The game archive** is a `.sdz` in `games/`, which is a zip; its
//!   `modinfo.lua` carries the name the engine indexes it under, and that
//!   name is what `GameType` has to be.
//! - **The roster** is `units/*.lua` in that same archive - 275 of them in a
//!   Steam install - each a plain Lua table whose key is the internal name and
//!   whose `name` field is what a player calls it.
//! - **The AIs** are directories under `AI/Skirmish/`.
//!
//! Everything here degrades rather than fails: a missing archive means an
//! empty list and a sentence saying so, not an error that stops the editor
//! opening. The authority is always the install, never this file.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A game archive the engine would index, and the name it indexes it under.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameArchive {
    /// What `GameType` has to be set to. Not the filename.
    pub name: String,
    pub path: PathBuf,
}

/// One placeable unit, as the game defines it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitDef {
    /// The internal name, which is what a start script places.
    pub name: String,
    /// What a player calls it - "Glaive" for `cloakraid`.
    pub title: String,
    pub description: String,
    /// For grouping the palette: the factory that builds it where one does,
    /// and a name-derived group otherwise.
    pub group: String,
}

// ------------------------------------------------------------- engines -----

/// Platform subfolder ZK files engines under. Mirrors `install::engine_platform`,
/// which is private to that module.
fn platform() -> &'static str {
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

/// Compare two engine version strings newest-first.
///
/// Versions are `2025.06.21`, or `105.1.1-2511-g1234567 maintenance`. Comparing
/// them as strings puts `105` above `2025`, so the numeric runs are compared as
/// numbers and everything else lexically. Purely so "the newest one" means what
/// a person means by it.
fn version_key(version: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    let mut rest = version;
    while !rest.is_empty() {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        rest = &rest[digits.len()..];
        let text: String = rest.chars().take_while(|c| !c.is_ascii_digit()).collect();
        rest = &rest[text.len()..];
        out.push((digits.parse().unwrap_or(0), text));
    }
    out
}

/// Every engine version installed, newest first.
///
/// Both layouts `install.rs` knows about are probed, because a version is only
/// real if the binary is actually there - an empty directory left behind by a
/// failed download would otherwise be offered and then fail at launch.
pub fn engine_versions(root: &Path) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let exe = engine_exe();
    for base in [root.join("engine").join(platform()), root.join("engine")] {
        let Ok(entries) = std::fs::read_dir(&base) else { continue };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let has_exe = entry.path().join(exe).is_file()
                || entry.path().join("bin").join(exe).is_file();
            if !has_exe {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
    found
}

// ---------------------------------------------------------------- games -----

/// Pull `key = value` out of a Lua table body.
///
/// Deliberately not a Lua parser. Zero-K's `modinfo.lua` and unit definitions
/// are flat tables of literals written by hand, and the two forms that appear
/// are `[[text]]` and `"text"`. Anything cleverer would be a parser to maintain
/// for no gain; anything that finds nothing returns `None` and the caller says
/// so rather than guessing.
fn lua_field(source: &str, key: &str) -> Option<String> {
    let lowered = source.to_ascii_lowercase();
    let needle = key.to_ascii_lowercase();
    let mut at = 0;
    while let Some(i) = lowered[at..].find(&needle) {
        let start = at + i;
        at = start + needle.len();
        // A key, not a substring of a longer one.
        let before = lowered[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let after = &source[at..];
        let after = after.trim_start();
        let Some(after) = after.strip_prefix('=') else { continue };
        let after = after.trim_start();
        if let Some(rest) = after.strip_prefix("[[") {
            return rest.find("]]").map(|e| rest[..e].trim().to_string());
        }
        for quote in ['"', '\''] {
            if let Some(rest) = after.strip_prefix(quote) {
                return rest.find(quote).map(|e| rest[..e].trim().to_string());
            }
        }
    }
    None
}

/// The name the engine indexes an archive under.
///
/// Spring appends the declared version to the declared name unless the name
/// already carries it, which is why `User Interface Tutorial r22` is one field
/// and `Zero-K` plus `v1.14.8.0` is two. `GameType` has to match this exactly
/// or the engine reports an unknown game.
pub fn archive_name(modinfo: &str) -> Option<String> {
    let name = lua_field(modinfo, "name")?;
    if name.is_empty() {
        return None;
    }
    match lua_field(modinfo, "version") {
        Some(version) if !version.is_empty() && !name.ends_with(&version) => {
            Some(format!("{name} {version}"))
        }
        _ => Some(name),
    }
}

/// Read one file out of a `.sdz`.
fn read_from_archive(path: &Path, wanted: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    // Case-insensitively, because Spring's VFS is and archives disagree.
    let index = (0..zip.len()).find(|i| {
        zip.by_index_raw(*i)
            .map(|e| e.name().eq_ignore_ascii_case(wanted))
            .unwrap_or(false)
    })?;
    let mut entry = zip.by_index(index).ok()?;
    let mut out = String::new();
    entry.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Every game archive in the install, with the name `GameType` needs.
///
/// `.sd7` archives are skipped rather than mis-reported: they are 7-zip, this
/// reads zip, and offering a name we could not actually read would produce a
/// script that fails at the whistle. A Steam install ships `zk-stable.sdz`,
/// which is the case that matters.
pub fn game_archives(root: &Path) -> Vec<GameArchive> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("games")) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if !matches!(path.extension(), Some(e) if e.eq_ignore_ascii_case("sdz")) {
            continue;
        }
        let Some(modinfo) = read_from_archive(&path, "modinfo.lua") else { continue };
        if let Some(name) = archive_name(&modinfo) {
            out.push(GameArchive { name, path });
        }
    }
    // The base game before its mutators: a mission mutator declares `modtype 0`
    // and is not what somebody building a scenario means by "the game".
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The base game, as opposed to a mutator sitting beside it.
///
/// Zero-K's own archive is the one whose name starts with "Zero-K"; a mission
/// mutator is named after the mission. When nothing matches, the first archive
/// is returned rather than none, because an install with exactly one game in it
/// should not need the author to know this rule.
pub fn base_game(root: &Path) -> Option<GameArchive> {
    let archives = game_archives(root);
    archives
        .iter()
        .find(|a| a.name.to_ascii_lowercase().starts_with("zero-k"))
        .or_else(|| archives.first())
        .cloned()
}

// ----------------------------------------------------------------- maps -----

/// Maps present on this machine, by the name a start script uses.
///
/// Zero-K downloads maps on demand through the lobby, so the catalogue lists
/// far more than any install actually has - 343 against a handful. A start
/// script naming a map that is not here fails at the engine with an error about
/// the archive, which is a poor way to learn that you needed to play the map
/// once first.
///
/// The name is the filename without its extension, which is what Spring indexes
/// a map archive under when its own metadata is unreadable, and what the
/// catalogue's names correspond to once underscores are spaces.
pub fn installed_maps(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("maps")) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_map = matches!(path.extension().and_then(|e| e.to_str()), Some(e)
            if e.eq_ignore_ascii_case("sd7")
                || e.eq_ignore_ascii_case("sdz")
                || e.eq_ignore_ascii_case("smf"));
        if !is_map {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(stem.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Whether a catalogue name matches an installed archive.
///
/// The catalogue says "Comet Catcher Redux" and the file on disk is
/// `comet_catcher_redux.sd7`, so neither case nor the spaces can be trusted.
pub fn map_is_installed(installed: &[String], name: &str) -> bool {
    let normal = |s: &str| s.to_ascii_lowercase().replace([' ', '_', '-'], "");
    let wanted = normal(name);
    installed.iter().any(|m| normal(m) == wanted)
}

// ------------------------------------------------------------------ AIs -----

/// Skirmish AIs the install can run, by the short name a start script uses.
pub fn skirmish_ais(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("AI").join("Skirmish")) else { return out };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            out.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------- units -----

/// Zero-K's roster, vendored so the editor has real unit names before an
/// install has been located. Regenerate with `tools/gen-roster.py`.
///
/// The installed game always wins: `read_unit_defs` reads the same information
/// out of `zk-stable.sdz` and replaces this entirely. This is the fallback, and
/// it exists because the list it replaced was invented - twenty-three
/// *Balanced Annihilation* names, not one of which Zero-K defines.
pub const ROSTER_PIN: &str = "32c1eca4e75c8c49161edda37ef5c391b9c01371";
const ROSTER: &str = include_str!("roster.json");

/// The vendored roster, parsed.
pub fn vendored_units() -> Vec<UnitDef> {
    serde_json::from_str(ROSTER).unwrap_or_default()
}

/// The internal name a unit definition registers itself under.
///
/// Zero-K writes `return { cloakraid = { ... } }`, so the name is the table key
/// rather than a field. It agrees with the filename for 274 of 275 units and
/// `damagesinkrock.lua` defines `rocksink`, which is exactly the kind of unit
/// that would be silently unplaceable if this read the filename instead.
fn table_key(source: &str) -> Option<String> {
    let after = source.find("return")?;
    let rest = source[after + "return".len()..].trim_start();
    let rest = rest.strip_prefix('{')?.trim_start();
    let key: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!key.is_empty()).then_some(key)
}

/// The units a definition says it can build.
fn build_options(source: &str) -> Vec<String> {
    let Some(at) = source.find("buildoptions") else { return Vec::new() };
    let rest = &source[at..];
    let Some(open) = rest.find('{') else { return Vec::new() };
    let Some(close) = rest[open..].find('}') else { return Vec::new() };
    let body = &rest[open..open + close];
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = body[at..].find("[[") {
        let start = at + i + 2;
        let Some(e) = body[start..].find("]]") else { break };
        out.push(body[start..start + e].trim().to_string());
        at = start + e;
    }
    out
}

/// Where a builder ranks when it claims a unit for its group.
///
/// Load-bearing rather than tidy: `athena` builds a 22-unit cross-section drawn
/// from six different factories, so ranking builders alphabetically lets it
/// absorb six of the Cloakbot Factory's eleven and leave the group a player
/// knows by name holding five. Factories first, then their plates, then
/// everything else.
fn builder_rank(name: &str) -> u8 {
    if name.starts_with("factory") {
        0
    } else if name.starts_with("plate") {
        1
    } else {
        2
    }
}

/// Groups for what no builder claims - half the roster, and the half a scenario
/// most wants: commanders, turrets, economy.
///
/// Unlike grouping by builder this taxonomy is ours rather than the game's,
/// which is why it runs second and only over what is left over. Zero-K names
/// these systematically enough for the prefixes to hold.
const BY_NAME: &[(&str, &str)] = &[
    ("factory", "Factories"),
    ("plate", "Factories"),
    ("turret", "Defences"),
    ("energy", "Economy"),
    ("static", "Support Structures"),
    ("chicken", "Chickens"),
    ("strider", "Striders"),
    ("dbg_", "Test and debug"),
    ("fakeunit", "Test and debug"),
    ("tiptest", "Test and debug"),
    ("empiricaldps", "Test and debug"),
    ("damagesink", "Test and debug"),
];

fn group_by_name(name: &str) -> &'static str {
    for (prefix, label) in BY_NAME {
        if name.starts_with(prefix) {
            return label;
        }
    }
    // Every commander carries `com`, and no other unit does.
    if name.contains("com") {
        return "Commanders";
    }
    "Other"
}

/// Every unit the game defines, read out of its archive.
///
/// Falls back to nothing rather than to the vendored roster: a caller that
/// asked for the *installed* game's units should be told the archive could not
/// be read, not quietly handed a different answer.
pub fn read_unit_defs(archive: &Path) -> Result<Vec<UnitDef>, String> {
    let file = std::fs::File::open(archive)
        .map_err(|e| format!("could not open {}: {e}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| format!("{} is not a readable archive: {e}", archive.display()))?;

    // Indices first, so the archive is not borrowed while it is being read.
    let wanted: Vec<usize> = (0..zip.len())
        .filter(|i| {
            zip.by_index_raw(*i)
                .map(|e| {
                    let n = e.name().to_ascii_lowercase();
                    n.starts_with("units/") && n.ends_with(".lua")
                })
                .unwrap_or(false)
        })
        .collect();

    let mut units: Vec<UnitDef> = Vec::with_capacity(wanted.len());
    let mut builders: Vec<(String, Vec<String>)> = Vec::new();
    for index in wanted {
        let Ok(mut entry) = zip.by_index(index) else { continue };
        let path = entry.name().to_ascii_lowercase();
        let mut source = String::new();
        if entry.read_to_string(&mut source).is_err() {
            continue;
        }
        let stem = path
            .rsplit('/')
            .next()
            .and_then(|f| f.strip_suffix(".lua"))
            .unwrap_or_default()
            .to_string();
        let Some(name) = table_key(&source).or(Some(stem)).filter(|n| !n.is_empty()) else {
            continue;
        };
        let options = build_options(&source);
        if !options.is_empty() {
            builders.push((name.clone(), options));
        }
        units.push(UnitDef {
            title: lua_field(&source, "name").unwrap_or_else(|| name.clone()),
            description: lua_field(&source, "description").unwrap_or_default(),
            group: String::new(),
            name,
        });
    }

    // Claim by builder, factories first.
    builders.sort_by_key(|(name, _)| (builder_rank(name), name.clone()));
    let titles: std::collections::HashMap<String, String> = units
        .iter()
        .map(|u| (u.name.clone(), u.title.clone()))
        .collect();
    let mut claimed: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (builder, options) in &builders {
        let label = titles.get(builder).cloned().unwrap_or_else(|| builder.clone());
        for built in options {
            claimed.entry(built.clone()).or_insert_with(|| label.clone());
        }
    }
    for unit in &mut units {
        unit.group = claimed
            .get(&unit.name)
            .cloned()
            .unwrap_or_else(|| group_by_name(&unit.name).to_string());
    }

    units.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    Ok(units)
}

/// "Other" and the debug units sort last: neither is what somebody opening the
/// palette is looking for.
fn sort_key(u: &UnitDef) -> (u8, &str, &str) {
    let rank = match u.group.as_str() {
        "Other" => 2,
        "Test and debug" => 1,
        _ => 0,
    };
    (rank, &u.group, &u.title)
}

// ------------------------------------------------------------- commands -----

/// Everything the editor needs to know about the install, in one call.
///
/// Assembled as a whole rather than as four commands because the editor cannot
/// usefully act on any one of them alone, and because each answer explains the
/// others: an empty roster with a named archive means a read failure, an empty
/// roster with no archive means Zero-K is not installed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub engines: Vec<String>,
    pub games: Vec<GameArchive>,
    pub ais: Vec<String>,
    /// Maps actually on this machine, as opposed to the 343 in the catalogue.
    pub maps: Vec<String>,
    /// The defaults the editor should start from, already chosen.
    pub engine: Option<String>,
    pub game: Option<String>,
}

/// The roster, and where it came from.
///
/// The source travels with the units because the two answers differ in ways an
/// author needs to know: the installed game is authoritative and matches what
/// will actually spawn, while the vendored copy is a pin that may be older than
/// the game on the machine.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Roster {
    pub source: String,
    pub units: Vec<UnitDef>,
}

pub fn game_info(root: &Path) -> GameInfo {
    let engines = engine_versions(root);
    let games = game_archives(root);
    GameInfo {
        engine: engines.first().cloned(),
        game: base_game(root).map(|g| g.name),
        ais: skirmish_ais(root),
        maps: installed_maps(root),
        engines,
        games,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_engine_is_the_newest_by_number_not_by_string() {
        // Sorted as strings, "105.1.1" beats "2025.06.21", and the editor would
        // default to an engine years older than the one the player uses.
        let mut versions = [
            "105.1.1-2511-g1234567 maintenance".to_string(),
            "2025.06.21".to_string(),
            "2024.12.01".to_string(),
        ];
        versions.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
        assert_eq!(versions[0], "2025.06.21");
        assert_eq!(versions[2], "105.1.1-2511-g1234567 maintenance");
    }

    #[test]
    fn an_archive_name_is_the_name_plus_the_version() {
        // This exact string is what GameType has to carry, and Spring builds it
        // by appending the version unless the name already ends with it.
        let modinfo = r#"local modinfo = {
	name = [[Zero-K]],
	shortname = [[ZK]],
	version = [[v1.14.8.0]],
}
return modinfo"#;
        assert_eq!(archive_name(modinfo).as_deref(), Some("Zero-K v1.14.8.0"));
    }

    #[test]
    fn a_name_that_already_carries_its_version_is_not_doubled() {
        // The mission mutator in our own fixtures is named this way, and its
        // start script says GameType=User Interface Tutorial r22.
        let modinfo = r#"local modinfo = {
	name        = [[User Interface Tutorial r22]],
	description = [[Mission Mutator]],
	version     = [[r22]],
}"#;
        assert_eq!(
            archive_name(modinfo).as_deref(),
            Some("User Interface Tutorial r22")
        );
    }

    #[test]
    fn an_archive_with_no_version_keeps_its_name() {
        assert_eq!(archive_name("{ name = \"Bare\" }").as_deref(), Some("Bare"));
    }

    #[test]
    fn quoted_and_bracketed_lua_strings_both_read() {
        assert_eq!(lua_field(r#"name = "Glaive","#, "name").as_deref(), Some("Glaive"));
        assert_eq!(lua_field("name = [[Glaive]],", "name").as_deref(), Some("Glaive"));
        assert_eq!(lua_field("name\t=\t[[Glaive]]", "name").as_deref(), Some("Glaive"));
    }

    #[test]
    fn a_key_is_not_matched_inside_a_longer_key() {
        // `unitname` contains `name`, and reading the wrong one gives every
        // unit the internal name as its title.
        let source = "unitname = [[cloakraid]],\n  name = [[Glaive]],";
        assert_eq!(lua_field(source, "name").as_deref(), Some("Glaive"));
        assert_eq!(lua_field(source, "unitname").as_deref(), Some("cloakraid"));
    }

    #[test]
    fn a_missing_field_is_absent_rather_than_empty() {
        assert_eq!(lua_field("{ name = [[x]] }", "buildpic"), None);
    }

    #[test]
    fn a_units_name_is_its_table_key_not_its_filename() {
        // damagesinkrock.lua defines `rocksink`. Reading the filename would
        // place a unit the engine has never heard of.
        assert_eq!(
            table_key("return { rocksink = {\n  name = [[Rock]],").as_deref(),
            Some("rocksink")
        );
        assert_eq!(
            table_key("return { cloakraid = {").as_deref(),
            Some("cloakraid")
        );
    }

    #[test]
    fn build_options_are_read_as_a_list() {
        let source = "  buildoptions = {\n    [[cloakcon]],\n    [[cloakraid]],\n  },";
        assert_eq!(build_options(source), vec!["cloakcon", "cloakraid"]);
        assert!(build_options("name = [[Glaive]]").is_empty());
    }

    #[test]
    fn a_factory_outranks_athena_when_both_build_a_unit() {
        /* Athena builds a cross-section of six factories' units. Ranked
           alphabetically it takes six of the Cloakbot Factory's eleven, and the
           palette group a player knows by name is left holding five. */
        assert!(builder_rank("factorycloak") < builder_rank("athena"));
        assert!(builder_rank("factorycloak") < builder_rank("platecloak"));
    }

    #[test]
    fn the_leftovers_are_grouped_by_the_names_zero_k_uses() {
        assert_eq!(group_by_name("turretlaser"), "Defences");
        assert_eq!(group_by_name("energysolar"), "Economy");
        assert_eq!(group_by_name("armcom1"), "Commanders");
        assert_eq!(group_by_name("commsupport1"), "Commanders");
        assert_eq!(group_by_name("dbg_m0r0"), "Test and debug");
        assert_eq!(group_by_name("zenith"), "Other");
    }

    #[test]
    fn the_vendored_roster_is_real_zero_k_units() {
        /* The list this replaced was Balanced Annihilation's: `armpw`,
           `corhlt`, `armmex`. Zero-K defines none of them, so every scenario
           built with that palette placed nothing at all. */
        let units = vendored_units();
        assert!(units.len() > 250, "only {} units", units.len());
        let named = |n: &str| units.iter().any(|u| u.name == n);
        assert!(named("cloakraid"), "Glaive is missing");
        assert!(named("armcom1"), "the Strike Commander is missing");
        assert!(named("turretlaser"), "the Lotus is missing");
        for invented in ["armpw", "corhlt", "armmex", "armsolar"] {
            assert!(!named(invented), "{invented} is not a Zero-K unit");
        }
        let glaive = units.iter().find(|u| u.name == "cloakraid").unwrap();
        assert_eq!(glaive.title, "Glaive");
        assert_eq!(glaive.group, "Cloakbot Factory");
    }

    #[test]
    fn a_catalogue_name_matches_the_file_on_disk() {
        // The catalogue says "Comet Catcher Redux"; the archive is
        // comet_catcher_redux.sd7. Neither case nor the spaces survive.
        let installed = vec!["comet_catcher_redux".to_string(), "Glacies 1.3".to_string()];
        assert!(map_is_installed(&installed, "Comet Catcher Redux"));
        assert!(map_is_installed(&installed, "Glacies 1.3"));
        assert!(!map_is_installed(&installed, "Some Other Map"));
    }

    #[test]
    fn nothing_here_fails_when_zero_k_is_absent() {
        // The editor has to open on a machine with no install, and say so,
        // rather than refusing to start.
        let nowhere = Path::new("/definitely/not/zero-k");
        assert!(engine_versions(nowhere).is_empty());
        assert!(game_archives(nowhere).is_empty());
        assert!(skirmish_ais(nowhere).is_empty());
        assert!(installed_maps(nowhere).is_empty());
        let info = game_info(nowhere);
        assert!(info.engine.is_none());
        assert!(info.game.is_none());
    }
}
