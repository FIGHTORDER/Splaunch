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

/// Pull a numeric `key = value` out of a Lua table body.
///
/// `modtype` is the field that says whether an archive is a game or a map, and
/// the one field read here that the engine writes unquoted. Older caches quote
/// it, so both forms are accepted.
fn lua_number(source: &str, key: &str) -> Option<i64> {
    let lowered = source.to_ascii_lowercase();
    let needle = key.to_ascii_lowercase();
    let mut at = 0;
    while let Some(i) = lowered[at..].find(&needle) {
        let start = at + i;
        at = start + needle.len();
        let before = lowered[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let after = source[at..].trim_start();
        let Some(after) = after.strip_prefix('=') else { continue };
        let after = after.trim_start();
        let after = after.strip_prefix('"').unwrap_or(after);
        let digits: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        if let Ok(n) = digits.parse() {
            return Some(n);
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

/// What the engine recorded about one archive.
#[derive(Debug)]
struct Cached {
    file: String,
    name: String,
    modtype: i64,
}

/// The closing brace matching the one at `open`, ignoring braces in strings.
///
/// Counting braces alone is not safe: a `description` is free text from whoever
/// packaged the archive, and one containing a brace would end the entry early
/// and take the rest of the file with it. Long-bracket strings hold Windows
/// paths and need the same skip.
fn block_end(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    if b.get(open) != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'[' if b.get(i + 1) == Some(&b'[') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b']' && b[i + 1] == b']') {
                    i += 1;
                }
                i += 1;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split one `ArchiveCache` into entries.
fn read_cache(text: &str, out: &mut Vec<Cached>) {
    let Some(list) = text.find("archives") else { return };
    let Some(open) = text[list..].find('{').map(|i| list + i) else { return };
    let Some(close) = block_end(text, open) else { return };
    let mut at = open + 1;
    while at < close {
        let Some(start) = text[at..close].find('{').map(|i| at + i) else { break };
        let Some(end) = block_end(text, start) else { break };
        let block = &text[start..=end];
        at = end + 1;
        let Some(data_at) = block
            .find("archivedata")
            .and_then(|i| block[i..].find('{').map(|j| i + j))
        else {
            continue;
        };
        let Some(data_end) = block_end(block, data_at) else { continue };
        let data = &block[data_at..=data_end];
        // The outer name is the file; the inner one is what the engine indexes
        // it under, which is the string a start script has to carry.
        let (Some(file), Some(name)) =
            (lua_field(&block[..data_at], "name"), lua_field(data, "name"))
        else {
            continue;
        };
        out.push(Cached {
            file,
            name,
            modtype: lua_number(data, "modtype").unwrap_or(-1),
        });
    }
}

/// Every archive the engine has indexed on this machine.
///
/// `games/` is only half the picture, and the missing half is the important
/// one. A rapid install, which is what both Zero-K's own installer and Shiro
/// produce, keeps the game itself as a `.sdp` in `packages/` whose contents
/// live in the pool. Scanning `games/` there finds every mod on the machine
/// except the game they are mods for, so `base_game` fell through to whatever
/// sorted first and the scenario compiled to a script naming a `GameType` that
/// was not Zero-K. On the machine this was found on, that was a racing mod.
///
/// The engine writes `cache/ArchiveCache<N>.lua` whenever it scans, and its
/// `name` is already the exact string `GameType` wants, for `.sd7` archives
/// this cannot open as well as for `.sdz`. `modtype` sorts them: 1 is a game,
/// 3 a map, 0 a mission mutator hidden from the mod list.
fn cached_archives(root: &Path) -> Vec<Cached> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("cache")) else { return out };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.starts_with("ArchiveCache") && f.ends_with(".lua"))
        })
        .collect();
    files.sort();
    for path in files {
        if let Ok(text) = std::fs::read_to_string(&path) {
            read_cache(&text, &mut out);
        }
    }
    out
}

/// Every game archive in the install, with the name `GameType` needs.
///
/// The engine's index first, because it is the only source that can see a rapid
/// package. The `games/` directory after it, for an install whose engine has
/// never run and so has written no cache yet.
///
/// `.sd7` archives are skipped by the directory scan rather than mis-reported:
/// they are 7-zip, this reads zip, and offering a name we could not actually
/// read would produce a script that fails at the whistle. The index carries no
/// such limit, having been written by the engine that did read them.
pub fn game_archives(root: &Path) -> Vec<GameArchive> {
    let mut out: Vec<GameArchive> = Vec::new();
    for c in cached_archives(root) {
        // 3 is a map. 0 is a mission mutator, which is still a launchable game.
        if c.modtype != 1 && c.modtype != 0 {
            continue;
        }
        /* The cache records an absolute path, which is wrong the moment an
           install is moved, so the file is looked for under this root instead.
           That also drops entries the engine indexed and somebody has since
           deleted, which is the direction that matters: a name we cannot back
           with a file compiles into a script that fails at the whistle. */
        let Some(path) = ["packages", "games"]
            .iter()
            .map(|d| root.join(d).join(&c.file))
            .find(|p| p.exists())
        else {
            continue;
        };
        out.push(GameArchive { name: c.name, path });
    }
    if let Ok(entries) = std::fs::read_dir(root.join("games")) {
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
    }
    // The base game before its mutators: a mission mutator declares `modtype 0`
    // and is not what somebody building a scenario means by "the game".
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
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
/// Filenames are kept alongside, without their extension, because a map
/// downloaded since the engine last scanned is in `maps/` and not yet in the
/// index. That is the safe direction: at worst a name matches twice.
pub fn installed_maps(root: &Path) -> Vec<String> {
    /* The engine's index first, because it holds the name the engine actually
       indexes the map under, and a start script has to carry that exactly.
       `icy_crater_v4.sd7` is "Icy Crater v4" inside, and no amount of
       normalising a filename recovers those spaces. */
    let mut out: Vec<String> = cached_archives(root)
        .into_iter()
        .filter(|c| c.modtype == 3 && root.join("maps").join(&c.file).exists())
        .map(|c| c.name)
        .collect();
    let Ok(entries) = std::fs::read_dir(root.join("maps")) else {
        out.sort();
        out.dedup();
        return out;
    };
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

fn normal(s: &str) -> String {
    s.to_ascii_lowercase().replace([' ', '_', '-', '.'], "")
}

/// The installed archive a scenario's map name refers to, named as the engine
/// names it.
///
/// Two things stand between the two strings. The catalogue says "Comet Catcher
/// Redux" and the file on disk is `comet_catcher_redux.sd7`, so neither case
/// nor the spaces can be trusted. And a scenario carries whatever its author
/// typed, which is often the map without the version the archive carries:
/// "Comet Catcher Redux" against "Comet Catcher Redux v3.1".
///
/// `Mapname` in a start script has to be the archive's own name or the engine
/// stops with an error about the map, so the near miss is corrected here rather
/// than becoming a failed launch.
///
/// A prefix only counts when exactly one archive matches it. Two versions of
/// the same map installed side by side is a real situation, and starting the
/// wrong one silently is worse than saying so.
pub fn resolve_map(installed: &[String], name: &str) -> Option<String> {
    let wanted = normal(name);
    if wanted.is_empty() {
        return None;
    }
    if let Some(exact) = installed.iter().find(|m| normal(m) == wanted) {
        return Some(exact.clone());
    }
    let mut near = installed.iter().filter(|m| normal(m).starts_with(&wanted));
    let first = near.next()?;
    // Two archives answer to this name, so neither of them is the answer.
    if near.next().is_some() {
        return None;
    }
    Some(first.clone())
}

/// Whether a catalogue name matches an installed archive.
pub fn map_is_installed(installed: &[String], name: &str) -> bool {
    resolve_map(installed, name).is_some()
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

/// Whether Zero-K would call this unit a commander.
///
/// The compiler needs to know, because Zero-K spawns a team's commander itself
/// rather than taking it from `extrastartunits` - so a placed commander says
/// *where the team starts*, not what to spawn. Answered by the same rule the
/// palette groups by, rather than by a second list that could drift from it.
pub fn is_commander(name: &str) -> bool {
    group_by_name(name) == "Commanders"
}

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

    /// Three entries verbatim from a real `ArchiveCache20.lua`, trimmed: the
    /// game as a rapid package, a map, and a mission mutator. The description
    /// carries a brace it does not have in life, because that is the input that
    /// breaks a parser which only counts them.
    const CACHE: &str = r#"local archiveCache = {

	internalver = 20,

	archives = {  -- count = 3
		{
			name = "06860629e67e11ef60760893bbfb60d5.sdp",
			path = [[C:\Users\me\AppData\Roaming\info.zero-k.shiro\zk\packages\]],
			modified = "1787607601",
			archivedata = {
				description = "Zero-K {the good one}",
				game = "Zero-K",
				modtype = 1,
				mutator = "1",
				name = "Zero-K v1.14.8.0",
				name_pure = "Zero-K",
				shortgame = "ZK",
				shortname = "ZK",
				version = "v1.14.8.0",
			},
		},
		{
			name = "AlienDesert.sd7",
			path = [[C:\Users\me\AppData\Roaming\info.zero-k.shiro\zk\maps\]],
			modified = "1787792768",
			archivedata = {
				mapfile = "maps/AlienDesert.smf",
				modtype = 3,
				name = "AlienDesert",
				name_pure = "AlienDesert",
			},
		},
		{
			name = "quicktutorial.sdz",
			path = [[C:\Users\me\AppData\Roaming\info.zero-k.shiro\zk\games\]],
			modified = "1787607601",
			archivedata = {
				description = "Mission Mutator",
				modtype = 0,
				name = "Quick Tutorial r1",
				name_pure = "Quick Tutorial",
			},
		},
	},
}"#;

    #[test]
    fn the_engines_index_names_a_rapid_package_the_way_gametype_needs() {
        // The whole point. `games/` cannot see this archive at all: it is a
        // `.sdp` in `packages/`, and its contents are in the pool.
        let mut out = Vec::new();
        read_cache(CACHE, &mut out);
        let zk = out.iter().find(|c| c.modtype == 1).expect("a game");
        assert_eq!(zk.name, "Zero-K v1.14.8.0");
        assert_eq!(zk.file, "06860629e67e11ef60760893bbfb60d5.sdp");
    }

    #[test]
    fn a_map_is_told_apart_from_a_game_by_modtype() {
        let mut out = Vec::new();
        read_cache(CACHE, &mut out);
        assert_eq!(out.len(), 3);
        let map = out.iter().find(|c| c.name == "AlienDesert").expect("a map");
        assert_eq!(map.modtype, 3);
        let mutator = out.iter().find(|c| c.modtype == 0).expect("a mutator");
        assert_eq!(mutator.name, "Quick Tutorial r1");
    }

    #[test]
    fn a_brace_inside_a_description_does_not_end_the_entry() {
        // Counting braces alone reads the first entry as ending inside its own
        // description, and every entry after it is lost.
        let mut out = Vec::new();
        read_cache(CACHE, &mut out);
        assert_eq!(out.len(), 3, "a brace in free text swallowed the rest");
    }

    #[test]
    fn a_windows_path_in_a_long_string_is_not_read_as_a_field() {
        // `path` is a long-bracket string full of backslashes, and it sits
        // between the two `name` fields an entry carries.
        let mut out = Vec::new();
        read_cache(CACHE, &mut out);
        assert!(out.iter().all(|c| !c.file.contains('\\')), "{out:?}",);
    }

    #[test]
    fn modtype_reads_whether_it_is_quoted_or_not() {
        assert_eq!(lua_number("modtype = 1,", "modtype"), Some(1));
        assert_eq!(lua_number("modtype = \"3\",", "modtype"), Some(3));
        assert_eq!(lua_number("name = \"x\",", "modtype"), None);
        // `mutator = "1"` must not answer for `modtype`.
        assert_eq!(lua_number("mutator = \"1\",", "modtype"), None);
    }

    /// Against a real install, when one is pointed at.
    ///
    /// Ignored by default because CI has no Zero-K. Run it with the data
    /// directory in `SPLAUNCH_TEST_ZK_ROOT`:
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture
    /// ```
    ///
    /// This is the test that would have caught the launch blocker. Every unit
    /// test above passed while `base_game` was returning a racing mod, because
    /// no fixture had a rapid install in it.
    #[test]
    #[ignore = "needs a Zero-K install in SPLAUNCH_TEST_ZK_ROOT"]
    fn a_real_install_yields_a_game_the_engine_would_recognise() {
        let Ok(root) = std::env::var("SPLAUNCH_TEST_ZK_ROOT") else {
            panic!("set SPLAUNCH_TEST_ZK_ROOT to a Zero-K data directory");
        };
        let root = PathBuf::from(root);
        let info = game_info(&root);
        println!("engine: {:?}", info.engine);
        println!("game:   {:?}", info.game);
        println!("games:  {:?}", info.games.iter().map(|g| &g.name).collect::<Vec<_>>());
        println!("maps:   {} installed", info.maps.len());
        assert!(info.engine.is_some(), "no engine found under {}", root.display());
        let game = info.game.expect("no game found");
        assert!(
            game.to_ascii_lowercase().starts_with("zero-k"),
            "the game to launch came out as {game:?}, which is not Zero-K"
        );
    }

    #[test]
    fn a_map_name_without_its_version_still_finds_the_archive() {
        // What the shipped example does. The engine wants the archive's own
        // name, and an author types the map's.
        let installed = vec!["Comet Catcher Redux v3.1".to_string()];
        assert_eq!(
            resolve_map(&installed, "Comet Catcher Redux").as_deref(),
            Some("Comet Catcher Redux v3.1")
        );
    }

    #[test]
    fn a_filename_matches_the_catalogue_name_it_came_from() {
        let installed = vec!["icy_crater_v4".to_string()];
        assert!(map_is_installed(&installed, "Icy Crater v4"));
    }

    #[test]
    fn the_engines_own_name_is_preferred_over_the_filename() {
        // Both are offered, and the one a start script can use wins.
        let installed = vec!["Icy Crater v4".to_string(), "icy_crater_v4".to_string()];
        assert_eq!(
            resolve_map(&installed, "Icy Crater v4").as_deref(),
            Some("Icy Crater v4")
        );
    }

    #[test]
    fn two_versions_of_a_map_refuse_to_answer_to_the_bare_name() {
        // Guessing here starts the wrong map with no way to tell.
        let installed = vec!["Tabula v6.1".to_string(), "Tabula v6.2".to_string()];
        assert_eq!(resolve_map(&installed, "Tabula"), None);
        // Naming one of them exactly is still unambiguous.
        assert_eq!(
            resolve_map(&installed, "Tabula v6.2").as_deref(),
            Some("Tabula v6.2")
        );
    }

    #[test]
    fn a_map_that_is_not_there_is_not_invented() {
        let installed = vec!["Icy Crater v4".to_string()];
        assert_eq!(resolve_map(&installed, "Comet Catcher Redux"), None);
        assert_eq!(resolve_map(&installed, ""), None);
    }

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
