//! Splaunch: Zero-K scenarios, and the start script they compile to.
//!
//! How one is made, and how the
//! game reads it. The finding this module is built on: **a Zero-K scenario's most
//! portable form is a start script, not a file format.** The engine reads `script.txt`; units, teams, AIs and modoptions are
//! all expressible there against unmodified Zero-K, with no archive to build and
//! no server to publish to.
//!
//! The consequence is that "Test" is not a preview. It writes a script and
//! launches the real game into it, so there is no second renderer to build and
//! no fidelity gap to apologise for.
//!
//! The writer escapes rather than refuses. A lobby has to reject a name
//! containing `;` or `}` outright, because a server-issued name with a
//! delimiter in it would silently produce a different script than intended. A
//! scenario name is the author's own, and losing their semicolon beats refusing
//! to launch - so delimiters are removed and everything else is kept.

use serde::{Deserialize, Serialize};

use crate::customkey::{self, Table, Value};
use crate::customkey as ck;

/// One unit placed on the map.
///
/// The optional fields are the ones `mission_galaxy_campaign_battle.lua` reads
/// off a placed unit, checked against the gadget rather than taken from a
/// document. Every one of them is omitted from the payload when unset, because
/// the gadget branches on presence and a defaulted value is not the same as no
/// value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    /// Zero-K's unit name, e.g. `cloakraid`. Not validated here - the engine is
    /// the authority on what exists, and guessing would go stale.
    pub unit: String,
    pub team: u32,
    /// Map position in elmos.
    pub x: f32,
    pub z: f32,
    /// 0-3, quarter turns. Random if absent.
    #[serde(default)]
    pub facing: Option<u32>,
    /// 0.0 to 1.0. A half-built factory is a scenario premise all by itself.
    #[serde(default)]
    pub build_progress: Option<f32>,
    /// Veterancy, so a defending unit can be a hardened one.
    #[serde(default)]
    pub experience: Option<f32>,
    /// `hold`, `maneuver` or `roam`, as the game spells them.
    #[serde(default)]
    pub movestate: Option<u32>,
    /// Cannot be killed. For the thing the scenario is about.
    #[serde(default)]
    pub invincible: Option<bool>,
    /// Flattens the ground under it, so a building on a slope still sits flat.
    #[serde(default)]
    pub terraform_height: Option<f32>,
    /// Owned by Gaia rather than by a team: scenery that shoots back, or a
    /// neutral objective sitting between two players.
    #[serde(default)]
    pub neutral: bool,
    /// Points to walk, in order, for ever.
    ///
    /// The closest the modern mission system comes to scripted behaviour. The
    /// gadget turns the first point into a move and the rest into shift-queued
    /// patrols, so a two-point route is a sentry walking a line.
    #[serde(default)]
    pub patrol: Vec<[f32; 2]>,
    /// Patrol on the spot, facing the middle of the map. One click, no route.
    #[serde(default)]
    pub self_patrol: bool,
    /// Only exists at or above this difficulty (1-3).
    #[serde(default)]
    pub difficulty_at_least: Option<u32>,
    /// Only exists at or below this difficulty (1-3).
    #[serde(default)]
    pub difficulty_at_most: Option<u32>,
}

/// A label on the map, shown to the player from the start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub x: f32,
    pub z: f32,
    pub text: String,
}

/// A team in the scenario. Team 0 is the player unless `ai` says otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: u32,
    pub ally: u32,
    /// None for the human player; otherwise the AI's short name.
    pub ai: Option<String>,
    /// "1 0 0". Left to the caller so the editor and the game agree on colours.
    pub colour: String,
}

/// A wreck, rock or other feature placed on the map.
///
/// Zero-K resurrects a feature whose name ends in `_dead` back into the unit
/// it came from, so placing `armcom_dead` leaves a reclaimable, rebuildable
/// wreck rather than scenery. The gadget wires that up on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub name: String,
    pub x: f32,
    pub z: f32,
    /// 0-3. Random if absent, which is what Zero-K does for scenery.
    #[serde(default)]
    pub facing: Option<u32>,
}

/// What an author is actually trying to say, rather than the fields it takes.
///
/// Zero-K's objectives are unit-count comparisons over time windows: 24 fields
/// whose useful combinations are not guessable from their names, and one of
/// which is spelled `comparisionType`. These seven cover the goals people
/// actually write, and each compiles to a combination read out of Zero-K's own
/// annotated reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Goal {
    /// Keep at least one of these alive to the deadline.
    SurviveUntil { seconds: u32, units: Vec<String> },
    /// Produce this many, counting ones that died on the way.
    BuildBy { unit: String, count: u32, seconds: u32 },
    /// Have this many at one moment. The satisfying set is frozen so
    /// overbuilding afterwards cannot pad it.
    HaveAtOnce { unit: String, count: u32 },
    /// None of the enemy's left by the deadline.
    DestroyAllBy { unit: String, seconds: u32 },
    /// Kill this many, however long it takes.
    KillCount { unit: String, count: u32 },
    /// Win the match before the clock runs out.
    WinBefore { seconds: u32 },
}

/// One objective: what the player is told, and what the game checks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub description: String,
    pub goal: Goal,
}

/// What losing looks like, for one side.
///
/// Indexed by allyteam in the payload. `vitalUnitTypes` is the usual one: lose
/// every commander and the mission ends, which is what a player expects and
/// what the campaign does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Defeat {
    pub ally: u32,
    /// Losing all of these loses the game for this side.
    #[serde(default)]
    pub vital_units: Vec<String>,
    /// A hard clock. Omitted when absent rather than sent as zero.
    #[serde(default)]
    pub lose_after_seconds: Option<u32>,
}

/// How wide the map is, in elmos.
///
/// Spring maps are `size * 512` elmos on a side, and the size is not knowable
/// from the name - it comes from the map's own header. The editor carries it so
/// placements mean the same thing on both sides of the bridge, and an author
/// can correct it when the catalogue is wrong.
pub const DEFAULT_MAP_ELMOS: u32 = 8 * 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    /// Bumped when the on-disk shape changes in a way an older Splaunch could
    /// not read. A scenario file outlives the version that wrote it.
    #[serde(default = "current_format")]
    pub format_version: u32,
    pub name: String,
    pub map: String,
    pub game: String,
    pub teams: Vec<Team>,
    pub units: Vec<Placed>,
    /// Free-text objectives, shown in the briefing alongside the checked ones.
    /// Kept because not every intention is a unit count, and a sentence beats
    /// contorting one into a comparison.
    pub objectives: Vec<String>,
    /// Objectives the game actually evaluates.
    #[serde(default)]
    pub goals: Vec<Objective>,
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Shown before the match starts, in Zero-K's briefing window.
    #[serde(default)]
    pub briefing: Option<String>,
    /// What losing means, per side.
    #[serde(default)]
    pub defeat: Vec<Defeat>,
    /// The map's width in elmos.
    #[serde(default = "default_map_elmos")]
    pub map_elmos: u32,
    /// Labels on the map, shown from the start.
    #[serde(default)]
    pub markers: Vec<Marker>,
    /// 1 easy, 2 normal, 3 hard. Gates the per-unit `difficultyAt*` fields, so
    /// one scenario can be three.
    #[serde(default = "default_difficulty")]
    pub difficulty: u32,
}

/// Zero-K's own default, from `mission_galaxy_campaign_battle.lua`.
pub const DEFAULT_DIFFICULTY: u32 = 2;

fn default_difficulty() -> u32 {
    DEFAULT_DIFFICULTY
}

/// The format version this build writes.
pub const FORMAT_VERSION: u32 = 1;

fn current_format() -> u32 {
    FORMAT_VERSION
}

fn default_map_elmos() -> u32 {
    DEFAULT_MAP_ELMOS
}

/// Anything a script value cannot contain.
///
/// Unlike `launch.rs`, this escapes rather than refuses: a scenario name is the
/// author's to choose, and losing their apostrophe is better than refusing to
/// launch. Delimiters are the exception - they change the script's shape, so
/// they are removed rather than represented.
fn escape(value: &str) -> String {
    value
        .chars()
        .filter(|c| !matches!(c, ';' | '{' | '}' | '\n' | '\r'))
        .collect()
}

fn key(out: &mut String, indent: &str, k: &str, v: impl std::fmt::Display) {
    out.push_str(&format!("{indent}{k}={v};\n"));
}

/// What is wrong with this scenario, in sentences a person can act on.
///
/// Returned rather than thrown so the editor can show a count before Test is
/// pressed: an invalid scenario should be visible while it is being made, not
/// after it fails to start.
pub fn problems(s: &Scenario) -> Vec<String> {
    let mut out = Vec::new();
    if s.map.trim().is_empty() {
        out.push("No map chosen.".into());
    }
    /* The one that used to be missing entirely. `GameType` names the archive
       the engine loads, and an empty one produces a script that is perfectly
       well-formed and starts nothing - which is a bad way to spend an evening.
       Splaunch never asked for it, because inside the lobby the server said. */
    if s.game.trim().is_empty() {
        out.push("No Zero-K version chosen - the game cannot start without one.".into());
    }
    if s.teams.is_empty() {
        out.push("No teams.".into());
    }
    if !s.teams.iter().any(|t| t.ai.is_none()) {
        out.push("No player team - somebody has to be you.".into());
    }
    let allies: std::collections::HashSet<u32> = s.teams.iter().map(|t| t.ally).collect();
    if allies.len() < 2 && !s.teams.is_empty() {
        out.push("Every team is on the same side, so the game ends immediately.".into());
    }
    for u in &s.units {
        if !s.teams.iter().any(|t| t.id == u.team) {
            out.push(format!("A {} belongs to team {}, which does not exist.", u.unit, u.team));
            break;
        }
    }
    if s.units.is_empty() {
        out.push("Nothing placed yet.".into());
    }
    /* Off the edge of the map the engine either clamps or drops the unit, and
       either way the scenario is not the one that was drawn. Worth catching
       here because the map size is itself a guess until an author corrects it. */
    let edge = s.map_elmos as f32;
    if let Some(stray) = s
        .units
        .iter()
        .find(|u| u.x < 0.0 || u.z < 0.0 || u.x > edge || u.z > edge)
    {
        out.push(format!(
            "A {} sits outside the map, at {}, {}. The map is {} elmos across.",
            stray.unit, stray.x as i64, stray.z as i64, s.map_elmos
        ));
    }
    for goal in &s.goals {
        if goal.description.trim().is_empty() {
            out.push("An objective has no description, so the player cannot read it.".into());
        }
    }
    out
}

/// Zero-K's comparison constants, from `mission_galaxy_campaign_battle.lua`.
const AT_LEAST: f64 = 1.0;
const AT_MOST: f64 = 2.0;

/// A list of unit names as the Lua array Zero-K expects.
fn unit_list(names: &[String]) -> Value {
    let mut list = Table::new();
    for name in names {
        list.push(ck::s(name));
    }
    ck::t(list)
}

/// One objective, as the field combination Zero-K evaluates.
///
/// The mapping is not invented: each combination is taken from the worked
/// examples in Zero-K's own `sample_planet.lua`, which is the only place the
/// interactions between `satisfy*`, `countRemovedUnits` and `lockUnitsOnSatisfy`
/// are written down.
fn goal_fields(objective: &Objective) -> Table {
    let mut table = Table::new();
    table.set("description", ck::s(&objective.description));

    match &objective.goal {
        Goal::WinBefore { seconds } => {
            // The one objective that is not a unit count.
            table.set("victoryByTime", ck::n(*seconds));
        }
        Goal::SurviveUntil { seconds, units } => {
            table.set("satisfyUntilTime", ck::n(*seconds));
            table.set("comparisionType", ck::n(AT_LEAST));
            table.set("targetNumber", ck::n(1));
            if !units.is_empty() {
                table.set("unitTypes", unit_list(units));
            }
        }
        Goal::BuildBy { unit, count, seconds } => {
            table.set("satisfyByTime", ck::n(*seconds));
            // Units that died on the way still count, or "build 5" would mean
            // "have 5 simultaneously" and fail for an unrelated reason.
            table.set("countRemovedUnits", ck::b(true));
            table.set("comparisionType", ck::n(AT_LEAST));
            table.set("targetNumber", ck::n(*count));
            table.set("unitTypes", unit_list(std::slice::from_ref(unit)));
        }
        Goal::HaveAtOnce { unit, count } => {
            table.set("satisfyOnce", ck::b(true));
            // Freeze the satisfying set, so building more afterwards cannot be
            // used to paper over losses.
            table.set("lockUnitsOnSatisfy", ck::b(true));
            table.set("comparisionType", ck::n(AT_LEAST));
            table.set("targetNumber", ck::n(*count));
            table.set("unitTypes", unit_list(std::slice::from_ref(unit)));
        }
        Goal::DestroyAllBy { unit, seconds } => {
            table.set("satisfyByTime", ck::n(*seconds));
            table.set("comparisionType", ck::n(AT_MOST));
            table.set("targetNumber", ck::n(0));
            table.set("enemyUnitTypes", unit_list(std::slice::from_ref(unit)));
        }
        Goal::KillCount { unit, count } => {
            table.set("satisfyOnce", ck::b(true));
            // Only the dead count, which is what makes this "kill" rather than
            // "have".
            table.set("onlyCountRemovedUnits", ck::b(true));
            table.set("comparisionType", ck::n(AT_LEAST));
            table.set("targetNumber", ck::n(*count));
            table.set("enemyUnitTypes", unit_list(std::slice::from_ref(unit)));
        }
    }
    table
}

/// The mission modoptions for a scenario, as `(key, encoded value)` pairs.
///
/// Zero-K's mission engine lives in the base game and is armed by a single
/// modoption, so a scenario needs no archive.
pub fn mission_modoptions(s: &Scenario) -> Vec<(String, String)> {
    let mut out = Vec::new();

    // Arms the mission engine. The value is only an identifier; the campaign
    // reports results against it, and nothing is listening for ours.
    out.push(("singleplayercampaignbattleid".into(), "splaunch".into()));

    if !s.goals.is_empty() {
        let mut list = Table::new();
        for objective in &s.goals {
            list.push(ck::t(goal_fields(objective)));
        }
        /* One key, not two. This used to send the same payload as
           `objectiveconfig` as well, on the theory that one drove the panel and
           the other the evaluation. `mission_galaxy_campaign_battle.lua` never
           reads `objectiveconfig` - the string does not occur in it - so the
           second copy was dead weight in a value that has a length limit. */
        out.push(("bonusobjectiveconfig".into(), customkey::encode(&list)));
    }

    /* Only when it is not Zero-K's own default. A scenario that does not use
       the difficultyAt* fields has nothing to say here, and a modoption that
       repeats the default is noise in a script somebody may have to read. */
    if s.difficulty != DEFAULT_DIFFICULTY {
        out.push(("planetmissiondifficulty".into(), s.difficulty.to_string()));
    }

    if !s.markers.is_empty() {
        let mut list = Table::new();
        for marker in &s.markers {
            let mut entry = Table::new();
            entry.set("x", ck::n(marker.x as f64));
            entry.set("z", ck::n(marker.z as f64));
            entry.set("text", ck::s(&marker.text));
            list.push(ck::t(entry));
        }
        out.push(("planetmissionmapmarkers".into(), customkey::encode(&list)));
    }

    if !s.features.is_empty() {
        let mut list = Table::new();
        for feature in &s.features {
            let mut entry = Table::new();
            entry.set("name", ck::s(&feature.name));
            entry.set("x", ck::n(feature.x as f64));
            entry.set("z", ck::n(feature.z as f64));
            if let Some(facing) = feature.facing {
                entry.set("facing", ck::n(facing));
            }
            list.push(ck::t(entry));
        }
        out.push(("featurestospawn".into(), customkey::encode(&list)));
    }

    /* The briefing, and the only place the free-text objectives go.
       They used to go nowhere at all: the editor collected them, the struct
       carried them, and `write_script` never read the field, so an author's
       objectives were dropped between pressing Test and the game starting.
       Zero-K's briefing window takes a name, a description and a list of tips,
       and a sentence that is not a unit count is exactly a tip. */
    let notes: Vec<&String> = s.objectives.iter().filter(|o| !o.trim().is_empty()).collect();
    let briefing = s.briefing.as_deref().map(str::trim).filter(|t| !t.is_empty());
    if briefing.is_some() || !notes.is_empty() {
        let mut info = Table::new();
        info.set("name", ck::s(&s.name));
        info.set("description", ck::s(briefing.unwrap_or("")));
        if !notes.is_empty() {
            let mut tips = Table::new();
            for note in notes {
                let mut tip = Table::new();
                tip.set("text", ck::s(note));
                tips.push(ck::t(tip));
            }
            info.set("tips", ck::t(tips));
        }
        out.push(("planetmissioninformationtext".into(), customkey::encode(&info)));
    }

    /* Defeat conditions, indexed by allyteam the way the gadget indexes them.
       Without any, a scenario ends only when one side has nothing left, which
       is a long way to lose a mission that was about one commander. */
    if !s.defeat.is_empty() {
        let mut list = Table::new();
        for defeat in &s.defeat {
            let mut entry = Table::new();
            if !defeat.vital_units.is_empty() {
                entry.set("vitalUnitTypes", unit_list(&defeat.vital_units));
            }
            if let Some(seconds) = defeat.lose_after_seconds {
                entry.set("loseAfterSeconds", ck::n(seconds));
            }
            list.set_index(defeat.ally as i64, ck::t(entry));
        }
        out.push(("defeatconditionconfig".into(), customkey::encode(&list)));
    }

    /* Gaia's units ride on the modoptions table rather than on a team, because
       that is where the gadget looks for them: it calls its own start-unit
       reader with `Spring.GetModOptions()` standing in for Gaia's custom keys. */
    for (key, value) in start_unit_keys(s.units.iter().filter(|u| u.neutral), "neutralstartunits") {
        out.push((key, value));
    }

    out
}

/// A team's placed units, as the custom keys Zero-K reads them from.
///
/// Chunked forty to a key because that is what Zero-K's own script builder
/// does - a start script value has a length limit, and forty is the number the
/// campaign settled on.
const START_UNITS_BLOCK: usize = 40;

/// One placed unit, as the gadget reads it.
///
/// `name` and not `unitDefName`: the gadget resolves a placed unit with
/// `UnitDefNames[unitData.name]`, and reserves `unitDefName` for retinue units,
/// which are a different feature entirely.
fn placed_fields(unit: &Placed) -> Table {
    let mut entry = Table::new();
    entry.set("name", ck::s(&unit.unit));
    entry.set("x", ck::n(unit.x as f64));
    entry.set("z", ck::n(unit.z as f64));
    // Each of these is written only when set. The gadget branches on the field
    // being present, so a defaulted zero is a different instruction from
    // silence - `buildProgress = 0` is an unbuilt husk, not a finished unit.
    if let Some(facing) = unit.facing {
        entry.set("facing", ck::n(facing));
    }
    if let Some(progress) = unit.build_progress {
        entry.set("buildProgress", ck::n(progress as f64));
    }
    if let Some(experience) = unit.experience {
        entry.set("experience", ck::n(experience as f64));
    }
    if let Some(movestate) = unit.movestate {
        entry.set("movestate", ck::n(movestate));
    }
    if let Some(true) = unit.invincible {
        entry.set("invincible", ck::b(true));
    }
    if let Some(height) = unit.terraform_height {
        entry.set("terraformHeight", ck::n(height as f64));
    }
    if let Some(at_least) = unit.difficulty_at_least {
        entry.set("difficultyAtLeast", ck::n(at_least));
    }
    if let Some(at_most) = unit.difficulty_at_most {
        entry.set("difficultyAtMost", ck::n(at_most));
    }
    /* A route beats the on-the-spot patrol, because the gadget checks
       `commands`, then `patrolRoute`, then `selfPatrol` and takes the first -
       so sending both would silently discard the one the author drew. */
    if unit.patrol.len() > 1 {
        let mut route = Table::new();
        for point in &unit.patrol {
            let mut pos = Table::new();
            pos.push(ck::n(point[0] as f64));
            pos.push(ck::n(point[1] as f64));
            route.push(ck::t(pos));
        }
        entry.set("patrolRoute", ck::t(route));
    } else if unit.self_patrol {
        entry.set("selfPatrol", ck::b(true));
    }
    entry
}

/// Chunk placed units into the numbered custom keys the gadget walks.
///
/// The gadget reads `<prefix>1`, `<prefix>2` and so on until one is missing, so
/// the numbering has to start at 1 and have no holes.
fn start_unit_keys<'a>(
    units: impl Iterator<Item = &'a Placed>,
    prefix: &str,
) -> Vec<(String, String)> {
    let mine: Vec<&Placed> = units.collect();
    let mut out = Vec::new();
    for (block, chunk) in mine.chunks(START_UNITS_BLOCK).enumerate() {
        let mut list = Table::new();
        for unit in chunk {
            list.push(ck::t(placed_fields(unit)));
        }
        out.push((format!("{prefix}_{}", block + 1), customkey::encode(&list)));
    }
    out
}

/// Compile to a Spring start script.
///
/// The shape is taken from a real one: `_missionScript.txt` inside Zero-K's own
/// `User Interface Tutorial r22.sdz`, which is what the old mission editor
/// emitted and what the engine still reads.
pub fn write_script(s: &Scenario, player: &str) -> Result<String, String> {
    if let Some(first) = problems(s).first() {
        return Err(first.clone());
    }

    let mut out = String::new();
    out.push_str("[GAME]\n{\n");
    key(&mut out, "\t", "Mapname", escape(&s.map));
    key(&mut out, "\t", "GameType", escape(&s.game));
    key(&mut out, "\t", "MyPlayerName", escape(player));
    // Local, hosted by us, nobody to wait for.
    key(&mut out, "\t", "IsHost", 1);
    key(&mut out, "\t", "OnlyLocal", 1);
    /* Spelled the way Zero-K's own mission script spells it. The engine's
       parser is case-insensitive, so this is not a fix - it is one less
       difference from the only script we know the engine accepts. */
    key(&mut out, "\t", "StartposType", 2);
    key(&mut out, "\t", "GameStartDelay", 0);
    key(&mut out, "\t", "NumRestrictions", 0);

    out.push_str("\t[MODOPTIONS]\n\t{\n");
    // Nothing a scenario does should count towards anybody's rating.
    key(&mut out, "\t\t", "noelo", 1);
    // The mission engine, its objectives, features and briefing. Not escaped:
    // these are base64 of our own making and contain no delimiter, and running
    // them through `escape` could only corrupt them.
    for (name, value) in mission_modoptions(s) {
        key(&mut out, "\t\t", &name, value);
    }
    out.push_str("\t}\n");

    // The human. One player, always index 0, on the first non-AI team.
    let human = s.teams.iter().find(|t| t.ai.is_none()).map(|t| t.id).unwrap_or(0);
    out.push_str("\t[PLAYER0]\n\t{\n");
    key(&mut out, "\t\t", "Name", escape(player));
    key(&mut out, "\t\t", "Team", human);
    out.push_str("\t}\n");

    for (i, t) in s.teams.iter().filter(|t| t.ai.is_some()).enumerate() {
        out.push_str(&format!("\t[AI{i}]\n\t{{\n"));
        key(&mut out, "\t\t", "Name", format!("AI {}", t.id));
        key(&mut out, "\t\t", "ShortName", escape(t.ai.as_deref().unwrap_or("NullAI")));
        key(&mut out, "\t\t", "Team", t.id);
        key(&mut out, "\t\t", "Host", 0);
        out.push_str("\t}\n");
    }

    for t in &s.teams {
        out.push_str(&format!("\t[TEAM{}]\n\t{{\n", t.id));
        key(&mut out, "\t\t", "TeamLeader", 0);
        key(&mut out, "\t\t", "AllyTeam", t.ally);
        key(&mut out, "\t\t", "RGBColor", escape(&t.colour));
        // Placed units ride on the team that owns them, which is how Zero-K
        // knows whose they are without a field saying so.
        let mine = s.units.iter().filter(|u| !u.neutral && u.team == t.id);
        for (name, value) in start_unit_keys(mine, "extrastartunits") {
            key(&mut out, "\t\t", &name, value);
        }
        out.push_str("\t}\n");
    }

    let mut allies: Vec<u32> = s.teams.iter().map(|t| t.ally).collect();
    allies.sort_unstable();
    allies.dedup();
    for a in allies {
        out.push_str(&format!("\t[ALLYTEAM{a}]\n\t{{\n"));
        key(&mut out, "\t\t", "NumAllies", 0);
        /* An empty start box, copied from Zero-K's own mission script, which
           carries exactly these four on every allyteam.

           `StartposType=2` is "choose in game", and a mission does not want the
           player choosing anything - the commander arrives through
           `extrastartunits` at the position the author placed it. What the
           engine does with a type-2 start and no boxes at all is not something
           we know, and this script has never been launched, so it matches the
           one that has rather than finding out. */
        for (name, value) in [
            ("StartRectTop", 0),
            ("StartRectBottom", 0),
            ("StartRectLeft", 1),
            ("StartRectRight", 1),
        ] {
            key(&mut out, "\t\t", name, value);
        }
        out.push_str("\t}\n");
    }

    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Scenario {
        Scenario {
            name: "Test".into(),
            map: "Comet Catcher Redux".into(),
            game: "Zero-K v1.14.8.0".into(),
            teams: vec![
                Team { id: 0, ally: 0, ai: None, colour: "0 0 1".into() },
                Team { id: 1, ally: 1, ai: Some("NullAI".into()), colour: "1 0 0".into() },
            ],
            units: vec![Placed {
                unit: "armcom1".into(),
                team: 0,
                x: 512.0,
                z: 512.0,
                ..Default::default()
            }],
            objectives: vec!["Destroy the enemy commander".into()],
            goals: vec![],
            features: vec![],
            briefing: None,
            defeat: vec![],
            format_version: FORMAT_VERSION,
            map_elmos: DEFAULT_MAP_ELMOS,
            markers: vec![],
            difficulty: DEFAULT_DIFFICULTY,
        }
    }


    use crate::customkey::{decode_as_the_game_does, to_lua};

    /// The value of `key=` inside a script section, unterminated semicolon and
    /// surrounding whitespace removed.
    fn value_of(section: &str, name: &str) -> Option<String> {
        section.lines().map(str::trim).find_map(|line| {
            let (k, v) = line.split_once('=')?;
            (k.trim() == name).then(|| v.trim_end_matches(';').to_string())
        })
    }

    /// A modoption, decoded the way Zero-K will decode it.
    fn modoption_lua(script: &str, name: &str) -> String {
        let block = script
            .split("[MODOPTIONS]")
            .nth(1)
            .expect("no [MODOPTIONS] section");
        let raw = value_of(block, name).unwrap_or_else(|| panic!("no {name} modoption"));
        String::from_utf8(decode_as_the_game_does(&raw))
            .unwrap_or_else(|e| panic!("{name} did not survive Zero-K's decoder: {e}"))
    }

    fn with_goals(goals: Vec<Objective>) -> Scenario {
        let mut sc = sample();
        sc.goals = goals;
        sc
    }

    #[test]
    fn the_mission_engine_is_armed() {
        // Without this one modoption the whole objective system stays asleep,
        // and a scenario with objectives would launch looking like a skirmish.
        let script = write_script(&sample(), "Qrow").unwrap();
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        assert_eq!(value_of(block, "singleplayercampaignbattleid").as_deref(), Some("splaunch"));
    }

    #[test]
    fn a_survival_goal_compiles_to_the_fields_zero_k_checks() {
        let sc = with_goals(vec![Objective {
            description: "Hold out for two minutes.".into(),
            goal: Goal::SurviveUntil { seconds: 120, units: vec!["armcom1".into()] },
        }]);
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "bonusobjectiveconfig");
        assert!(lua.contains("satisfyUntilTime=120"), "{lua}");
        assert!(lua.contains("comparisionType=1"), "{lua}");
        assert!(lua.contains("targetNumber=1"), "{lua}");
        assert!(lua.contains("armcom1"), "{lua}");
    }

    #[test]
    fn build_counts_the_dead_and_have_does_not() {
        // The difference between "build 5" and "have 5" is one flag, and
        // getting it wrong makes an objective that quietly cannot be completed.
        let build = with_goals(vec![Objective {
            description: "Build five Glaives.".into(),
            goal: Goal::BuildBy { unit: "cloakraid".into(), count: 5, seconds: 300 },
        }]);
        let lua = modoption_lua(&write_script(&build, "Qrow").unwrap(), "bonusobjectiveconfig");
        assert!(lua.contains("countRemovedUnits=true"), "{lua}");

        let have = with_goals(vec![Objective {
            description: "Have five Glaives.".into(),
            goal: Goal::HaveAtOnce { unit: "cloakraid".into(), count: 5 },
        }]);
        let lua = modoption_lua(&write_script(&have, "Qrow").unwrap(), "bonusobjectiveconfig");
        assert!(!lua.contains("countRemovedUnits"), "{lua}");
        assert!(lua.contains("lockUnitsOnSatisfy=true"), "{lua}");
    }

    #[test]
    fn killing_uses_the_enemys_units_not_ours() {
        let sc = with_goals(vec![Objective {
            description: "Kill three Reavers.".into(),
            goal: Goal::KillCount { unit: "cloakriot".into(), count: 3 },
        }]);
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "bonusobjectiveconfig");
        assert!(lua.contains("enemyUnitTypes"), "{lua}");
        assert!(!lua.contains("unitTypes={"), "counted our own units: {lua}");
        assert!(lua.contains("onlyCountRemovedUnits=true"), "{lua}");
    }

    #[test]
    fn a_question_mark_in_a_description_does_not_destroy_the_objectives() {
        /* Zero-K's decoder turns an unescaped '?' at the wrong offset into
           end-of-data, which loses every objective at once rather than just
           that one. The author should be
           able to ask a question. */
        for text in [
            "Can you hold the ridge?",
            "Ready? Set? Go?",
            "Halte den Grat fünf Minuten",
            "Продержись 5 минут",
        ] {
            let sc = with_goals(vec![Objective {
                description: text.into(),
                goal: Goal::WinBefore { seconds: 60 },
            }]);
            let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "bonusobjectiveconfig");
            assert!(lua.contains("victoryByTime=60"), "lost objectives for {text:?}: {lua}");
        }
    }

    #[test]
    fn features_reach_the_script() {
        let mut sc = sample();
        sc.features = vec![Feature { name: "armcom1_dead".into(), x: 100.0, z: 200.0, facing: Some(1) }];
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "featurestospawn");
        assert!(lua.contains("armcom1_dead"), "{lua}");
        assert!(lua.contains("x=100") && lua.contains("z=200"), "{lua}");
    }

    #[test]
    fn a_briefing_is_only_sent_when_there_is_something_to_read() {
        // Nothing to say: no briefing, and no notes either.
        let mut bare = sample();
        bare.objectives.clear();
        let script = write_script(&bare, "Qrow").unwrap();
        assert!(!script.contains("planetmissioninformationtext"));

        let mut sc = sample();
        sc.briefing = Some("The dam will not hold.".into());
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "planetmissioninformationtext");
        assert!(lua.contains("The dam will not hold."), "{lua}");
    }

    #[test]
    fn a_written_objective_reaches_the_player() {
        /* These used to go nowhere. The editor collected them, the struct
           carried them, and `write_script` never read the field - so an author
           typed objectives, pressed Test, and the game was told none of them.
           They ride in the briefing now, which is where a sentence that is not
           a unit count belongs. */
        let mut sc = sample();
        sc.objectives = vec!["Hold the northern ridge.".into(), "Do not lose the dam.".into()];
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "planetmissioninformationtext");
        assert!(lua.contains("Hold the northern ridge."), "{lua}");
        assert!(lua.contains("Do not lose the dam."), "{lua}");
        assert!(lua.contains("tips="), "notes should travel as briefing tips: {lua}");
    }

    #[test]
    fn nothing_is_sent_to_the_key_the_gadget_does_not_read() {
        /* Both `bonusobjectiveconfig` and `objectiveconfig` used to be sent,
           carrying identical payloads. `mission_galaxy_campaign_battle.lua`
           does not contain the string `objectiveconfig` at all, so the second
           copy did nothing except double the size of a value that has a
           length limit. */
        let sc = with_goals(vec![Objective {
            description: "Win.".into(),
            goal: Goal::WinBefore { seconds: 60 },
        }]);
        let script = write_script(&sc, "Qrow").unwrap();
        assert!(script.contains("bonusobjectiveconfig"));
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        assert!(value_of(block, "objectiveconfig").is_none(), "{block}");
    }

    #[test]
    fn a_scenario_with_no_zero_k_version_does_not_compile() {
        /* GameType names the archive the engine loads. Empty, the script is
           well-formed and starts nothing, which is the worst way for this to
           fail. Splaunch never asked for it because the lobby used to say. */
        let mut sc = sample();
        sc.game = String::new();
        assert!(problems(&sc).iter().any(|p| p.contains("Zero-K version")));
        let err = write_script(&sc, "Qrow").unwrap_err();
        assert!(err.contains("Zero-K version"), "{err}");
    }

    #[test]
    fn a_unit_off_the_edge_of_the_map_is_caught() {
        let mut sc = sample();
        sc.units[0].x = 99_000.0;
        assert!(problems(&sc).iter().any(|p| p.contains("outside the map")));
    }

    #[test]
    fn optional_unit_fields_are_omitted_rather_than_defaulted() {
        /* The gadget branches on a field being present. `buildProgress = 0` is
           an unbuilt husk, not a finished unit, so writing a default would
           change what spawns. */
        let plain = to_lua(&placed_fields(&Placed {
            unit: "cloakraid".into(),
            team: 0,
            x: 1.0,
            z: 2.0,
            ..Default::default()
        }));
        assert_eq!(plain, "{name=\"cloakraid\",x=1,z=2,}");

        let dressed = to_lua(&placed_fields(&Placed {
            unit: "armcom1".into(),
            team: 0,
            x: 1.0,
            z: 2.0,
            facing: Some(2),
            build_progress: Some(0.5),
            experience: Some(1.0),
            invincible: Some(true),
            ..Default::default()
        }));
        assert!(dressed.contains("facing=2"), "{dressed}");
        assert!(dressed.contains("buildProgress=0.5"), "{dressed}");
        assert!(dressed.contains("invincible=true"), "{dressed}");
    }

    #[test]
    fn a_patrol_route_travels_as_a_list_of_points() {
        let mut sc = sample();
        sc.units[0].patrol = vec![[100.0, 200.0], [800.0, 900.0]];
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let value = value_of(team0, "extrastartunits_1").unwrap();
        let lua = String::from_utf8(decode_as_the_game_does(&value)).unwrap();
        assert!(lua.contains("patrolRoute={[1]={[1]=100,[2]=200,},[2]={[1]=800,[2]=900,},}"), "{lua}");
    }

    #[test]
    fn a_route_beats_patrolling_on_the_spot() {
        /* The gadget takes the first of `commands`, `patrolRoute`, `selfPatrol`
           it finds, so sending both would silently discard the route the author
           drew and leave the unit turning circles instead. */
        let mut sc = sample();
        sc.units[0].patrol = vec![[10.0, 20.0], [30.0, 40.0]];
        sc.units[0].self_patrol = true;
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team0, "extrastartunits_1").unwrap())).unwrap();
        assert!(lua.contains("patrolRoute"), "{lua}");
        assert!(!lua.contains("selfPatrol"), "{lua}");

        // A one-point "route" is not a route, so it falls through.
        sc.units[0].patrol = vec![[10.0, 20.0]];
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team0, "extrastartunits_1").unwrap())).unwrap();
        assert!(!lua.contains("patrolRoute"), "{lua}");
        assert!(lua.contains("selfPatrol=true"), "{lua}");
    }

    #[test]
    fn difficulty_is_only_sent_when_it_is_not_the_games_own_default() {
        let script = write_script(&sample(), "Qrow").unwrap();
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        assert!(value_of(block, "planetmissiondifficulty").is_none(), "{block}");

        let mut sc = sample();
        sc.difficulty = 3;
        sc.units[0].difficulty_at_least = Some(3);
        let script = write_script(&sc, "Qrow").unwrap();
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        assert_eq!(value_of(block, "planetmissiondifficulty").as_deref(), Some("3"));
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team0, "extrastartunits_1").unwrap())).unwrap();
        assert!(lua.contains("difficultyAtLeast=3"), "{lua}");
    }

    #[test]
    fn markers_reach_the_map() {
        let mut sc = sample();
        sc.markers = vec![Marker { x: 900.0, z: 1200.0, text: "Here?".into() }];
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "planetmissionmapmarkers");
        assert!(lua.contains("x=900") && lua.contains("z=1200"), "{lua}");
        /* The question mark travels as the escape `\063` rather than as itself,
           which is the whole point of the transport: on the wire a `?` at the
           wrong offset encodes to `_`, which Zero-K's decoder rewrites to `=`
           and reads as end-of-data. Lua turns the escape back into `?` when it
           parses the literal, so the player sees the question mark. */
        assert!(lua.contains(r#"text="Here\063""#), "{lua}");
        assert!(!lua.contains("Here?"), "an unescaped ? would truncate: {lua}");
    }

    #[test]
    fn gaias_units_ride_on_the_modoptions_not_on_a_team() {
        /* The gadget reads neutral units by calling its own start-unit reader
           with `Spring.GetModOptions()` standing in for Gaia's custom keys. */
        let mut sc = sample();
        sc.units.push(Placed {
            unit: "turretlaser".into(),
            team: 0,
            x: 700.0,
            z: 700.0,
            neutral: true,
            ..Default::default()
        });
        let script = write_script(&sc, "Qrow").unwrap();
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        let value = value_of(block, "neutralstartunits_1").expect("no neutral units");
        let lua = String::from_utf8(decode_as_the_game_does(&value)).unwrap();
        assert!(lua.contains("turretlaser"), "{lua}");

        // And it is not also on the team that nominally owns it.
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let owned = value_of(team0, "extrastartunits_1").unwrap();
        let owned = String::from_utf8(decode_as_the_game_does(&owned)).unwrap();
        assert!(!owned.contains("turretlaser"), "{owned}");
    }

    #[test]
    fn a_scenario_survives_a_trip_through_a_file() {
        let mut sc = sample();
        sc.goals = vec![Objective {
            description: "Hold out.".into(),
            goal: Goal::SurviveUntil { seconds: 120, units: vec!["armcom1".into()] },
        }];
        sc.units[0].facing = Some(3);
        sc.units[0].invincible = Some(true);
        sc.defeat = vec![Defeat { ally: 0, vital_units: vec!["armcom1".into()], lose_after_seconds: None }];
        assert_eq!(from_json(&to_json(&sc).unwrap()).unwrap(), sc);
    }

    /// The scenario shipped in `examples/`, which is the one thing here that a
    /// person is invited to open and play.
    const EXAMPLE: &str = include_str!("../../examples/first-contact.splaunch");

    #[test]
    fn the_shipped_example_compiles_to_a_script() {
        /* A broken example is worse than none: it is the first thing somebody
           opens, and it would teach them the tool does not work.

           Its `game` is empty on purpose - which Zero-K it runs against is a
           property of the machine, not of the scenario, and the editor fills it
           in from the install. So that is what happens here too. */
        let mut sc = from_json(EXAMPLE).expect("the example does not parse");
        assert!(sc.game.is_empty(), "the example should not pin a Zero-K version");
        assert_eq!(
            problems(&sc),
            vec!["No Zero-K version chosen - the game cannot start without one."]
        );

        sc.game = "Zero-K v1.14.8.0".into();
        assert_eq!(problems(&sc), Vec::<String>::new(), "{:?}", problems(&sc));

        let script = write_script(&sc, "Player").unwrap();
        assert!(script.contains("Mapname=Comet Catcher Redux;"));
        assert!(script.contains("GameType=Zero-K v1.14.8.0;"));
        let lua = modoption_lua(&script, "bonusobjectiveconfig");
        assert!(lua.contains("corcom1"), "{lua}");
        assert!(lua.contains("satisfyUntilTime=900"), "{lua}");
        let brief = modoption_lua(&script, "planetmissioninformationtext");
        assert!(brief.contains("did not come back"), "{brief}");

        // It exercises the things a scenario can do, because an example that
        // shows one feature teaches one feature.
        let marks = modoption_lua(&script, "planetmissionmapmarkers");
        assert!(marks.contains("Their commander"), "{marks}");
        let team1 = script.split("[TEAM1]").nth(1).unwrap();
        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team1, "extrastartunits_1").unwrap())).unwrap();
        assert!(lua.contains("patrolRoute"), "the example has no patrol: {lua}");
        assert!(lua.contains("selfPatrol=true"), "{lua}");
        assert!(lua.contains("difficultyAtLeast=3"), "{lua}");
    }

    #[test]
    fn every_unit_in_the_example_is_a_unit_zero_k_has() {
        /* The failure this guards against is the one that made the old palette
           useless: a name that looks plausible, spawns nothing, and takes an
           evening to notice. The gadget resolves placed units and objective
           unit types through UnitDefNames and silently drops what it cannot
           find. */
        let sc = from_json(EXAMPLE).unwrap();
        let roster = crate::game::vendored_units();
        let known = |name: &str| roster.iter().any(|u| u.name == name);

        for unit in &sc.units {
            assert!(known(&unit.unit), "{} is not a Zero-K unit", unit.unit);
        }
        for feature in &sc.features {
            let base = feature.name.strip_suffix("_dead").unwrap_or(&feature.name);
            assert!(known(base), "{} is not a Zero-K unit's wreck", feature.name);
        }
        for objective in &sc.goals {
            for name in match &objective.goal {
                Goal::SurviveUntil { units, .. } => units.clone(),
                Goal::BuildBy { unit, .. }
                | Goal::HaveAtOnce { unit, .. }
                | Goal::DestroyAllBy { unit, .. }
                | Goal::KillCount { unit, .. } => vec![unit.clone()],
                Goal::WinBefore { .. } => vec![],
            } {
                assert!(known(&name), "objective names {name}, which is not a Zero-K unit");
            }
        }
        for defeat in &sc.defeat {
            for name in &defeat.vital_units {
                assert!(known(name), "defeat condition names {name}");
            }
        }
    }

    #[test]
    fn the_example_can_be_both_won_and_lost() {
        // A scenario with no way to lose is a sandbox, and one with no way to
        // win is a diorama. Both sides need a defeat condition.
        let sc = from_json(EXAMPLE).unwrap();
        let sides: std::collections::HashSet<u32> = sc.teams.iter().map(|t| t.ally).collect();
        for side in sides {
            assert!(
                sc.defeat.iter().any(|d| d.ally == side && !d.vital_units.is_empty()),
                "side {side} has no way to be beaten"
            );
        }
    }

    #[test]
    fn a_scenario_from_a_newer_splaunch_is_refused_by_name() {
        /* Half-reading it would drop whatever the newer version added, and the
           author would find out by playing a scenario missing an objective. */
        let mut sc = sample();
        sc.format_version = FORMAT_VERSION + 1;
        let err = from_json(&to_json(&sc).unwrap()).unwrap_err();
        assert!(err.contains("newer Splaunch"), "{err}");
    }

    #[test]
    fn an_older_scenario_without_the_new_fields_still_opens() {
        // Every field added after format 1 has a default, so a file written
        // before them reads rather than failing.
        let old = r#"{"name":"Old","map":"Comet Catcher Redux","game":"Zero-K v1.14.8.0",
            "teams":[{"id":0,"ally":0,"ai":null,"colour":"0 0 1"}],
            "units":[{"unit":"armcom1","team":0,"x":10,"z":10}],"objectives":[]}"#;
        let sc = from_json(old).unwrap();
        assert_eq!(sc.format_version, FORMAT_VERSION);
        assert_eq!(sc.map_elmos, DEFAULT_MAP_ELMOS);
        assert_eq!(sc.units[0].facing, None);
    }

    #[test]
    fn defeat_conditions_are_indexed_by_allyteam() {
        let mut sc = sample();
        sc.defeat = vec![Defeat {
            ally: 0,
            vital_units: vec!["armcom1".into()],
            lose_after_seconds: Some(600),
        }];
        let lua = modoption_lua(&write_script(&sc, "Qrow").unwrap(), "defeatconditionconfig");
        assert!(lua.starts_with("{[0]="), "not indexed by allyteam: {lua}");
        assert!(lua.contains("vitalUnitTypes"), "{lua}");
        assert!(lua.contains("loseAfterSeconds=600"), "{lua}");
    }

    #[test]
    fn many_units_are_chunked_the_way_zero_k_chunks_them() {
        // Forty to a key, because that is what Zero-K's own script builder does
        // and a start script value has a length limit.
        let mut sc = sample();
        sc.units = (0..85)
            .map(|i| Placed {
                unit: "cloakraid".into(),
                team: 0,
                x: i as f32,
                z: 0.0,
                ..Default::default()
            })
            .collect();
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        assert!(value_of(team0, "extrastartunits_1").is_some());
        assert!(value_of(team0, "extrastartunits_2").is_some());
        assert!(value_of(team0, "extrastartunits_3").is_some());
        assert!(value_of(team0, "extrastartunits_4").is_none());
    }

    #[test]
    fn a_scenario_compiles_to_a_script_the_engine_shape_matches() {
        let s = write_script(&sample(), "Qrow").unwrap();
        assert!(s.starts_with("[GAME]\n{\n"));
        assert!(s.contains("Mapname=Comet Catcher Redux;"));
        assert!(s.contains("OnlyLocal=1;"));
        assert!(s.contains("[PLAYER0]"));
        assert!(s.contains("[AI0]"));
        assert!(s.contains("ShortName=NullAI;"));
        assert!(s.contains("[TEAM0]") && s.contains("[TEAM1]"));
        assert!(s.contains("[ALLYTEAM0]") && s.contains("[ALLYTEAM1]"));
        assert!(s.contains("StartposType=2;"));
        assert!(s.trim_end().ends_with('}'));
    }

    #[test]
    fn braces_are_balanced() {
        // The engine's parser is not forgiving, and an unbalanced script fails
        // with a message about the wrong line.
        let s = write_script(&sample(), "Qrow").unwrap();
        assert_eq!(s.matches('{').count(), s.matches('}').count());
    }

    #[test]
    fn a_name_that_would_break_the_script_is_escaped_not_refused() {
        /* The join path refuses these, because a server-issued name never
           contains one. A scenario author's name is their own, and losing a
           semicolon beats refusing to launch. */
        let mut sc = sample();
        sc.map = "Weird; }Map{".into();
        let s = write_script(&sc, "Qrow").unwrap();
        assert!(s.contains("Mapname=Weird Map;"));
        assert_eq!(s.matches('{').count(), s.matches('}').count());
    }

    #[test]
    fn a_scenario_with_no_player_does_not_compile() {
        let mut sc = sample();
        sc.teams[0].ai = Some("NullAI".into());
        let err = write_script(&sc, "Qrow").unwrap_err();
        assert!(err.contains("player team"), "{err}");
    }

    #[test]
    fn problems_are_sentences_rather_than_codes() {
        let empty = Scenario {
            name: "".into(), map: "".into(), game: "".into(),
            teams: vec![], units: vec![], objectives: vec![],
            goals: vec![], features: vec![], briefing: None,
            defeat: vec![], format_version: FORMAT_VERSION,
            map_elmos: DEFAULT_MAP_ELMOS, markers: vec![],
            difficulty: DEFAULT_DIFFICULTY,
        };
        let p = problems(&empty);
        assert!(p.len() >= 3);
        for line in &p {
            assert!(line.ends_with('.'), "{line:?} is not a sentence");
        }
    }

    #[test]
    fn one_sided_scenarios_are_caught_before_launch() {
        // Two teams on the same allyteam ends the moment it starts, which is a
        // confusing way to find out you made a mistake.
        let mut sc = sample();
        sc.teams[1].ally = 0;
        assert!(problems(&sc).iter().any(|p| p.contains("same side")));
    }

    #[test]
    fn a_unit_on_a_team_that_does_not_exist_is_caught() {
        let mut sc = sample();
        sc.units[0].team = 7;
        assert!(problems(&sc).iter().any(|p| p.contains("does not exist")));
    }

    /// Zero-K's own mission script, lifted out of
    /// `games/User Interface Tutorial r22.sdz`. This is what the engine is
    /// known to accept, so it is the thing to be measured against.
    const REAL: &str = include_str!("fixtures/mission-script.txt");

    /// Every `[SECTION]` name, in order.
    fn sections(script: &str) -> Vec<String> {
        script
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('[') && l.ends_with(']'))
            .map(|l| l.trim_matches(['[', ']']).to_string())
            .collect()
    }

    /// Every `Key=` at any depth, lowercased.
    ///
    /// Split on `;` rather than on newlines, because newlines are not part of
    /// the engine's grammar: the real script puts four assignments and a
    /// closing brace on one line, and reading it a line at a time finds one key
    /// out of four.
    fn keys(script: &str) -> std::collections::HashSet<String> {
        script
            .split(';')
            .filter_map(|assignment| assignment.split_once('='))
            .map(|(k, _)| {
                k.trim_matches(|c: char| c.is_whitespace() || c == '{' || c == '}')
                    .rsplit(['\n', '\r'])
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase()
            })
            .filter(|k| !k.is_empty())
            .collect()
    }

    #[test]
    fn our_script_has_the_sections_the_engine_expects() {
        /* The single biggest unknown is still whether a script we write
           actually launches. Nothing here launches anything - that wants a
           machine with Zero-K on it - but a script missing a section the
           engine's own one has would fail for a reason we can find now rather
           than at the whistle. */
        let ours = write_script(&sample(), "Qrow").unwrap();
        let theirs = sections(REAL);
        let mine = sections(&ours);

        for want in ["GAME", "MODOPTIONS", "PLAYER0", "AI0", "TEAM0", "TEAM1", "ALLYTEAM0"] {
            assert!(theirs.iter().any(|s| s == want), "the real script has no [{want}]");
            assert!(mine.iter().any(|s| s == want), "ours has no [{want}]");
        }
    }

    #[test]
    fn our_script_sets_the_keys_the_engine_reads() {
        let ours = write_script(&sample(), "Qrow").unwrap();
        let theirs = keys(REAL);
        let mine = keys(&ours);

        /* Not every key - the real one carries mission-specific extras we have
           no business emitting. These are the ones that decide whether a local
           game starts at all, and every one of them is in theirs too. */
        for want in ["mapname", "gametype", "myplayername", "ishost", "onlylocal",
                     "gamestartdelay", "name", "team", "shortname", "allyteam",
                     "startpostype", "numallies",
                     "startrecttop", "startrectbottom", "startrectleft", "startrectright"] {
            assert!(theirs.contains(want), "the real script does not set {want}");
            assert!(mine.contains(want), "ours does not set {want}");
        }
    }

    #[test]
    fn our_script_parses_the_way_theirs_does() {
        /* Balanced braces, and every value terminated by a `;` before its
           section closes.

           Deliberately not a per-line rule: the real script puts four pairs on
           one line and closes the section on the same one -
           `StartRectTop=0;		StartRectBottom=0; ... }` - which is the engine
           telling us that newlines are not part of its grammar at all. Ours is
           formatted for a human to read, and that is free.

           A `=` inside a value is legal and has to stay legal: Zero-K's own
           custom keys are base64, and base64 pads with `=`. The engine splits
           an assignment at its first `=` and reads to the `;`, so that is what
           is checked here. This test used to treat a second `=` as a missing
           terminator, which was a stricter rule than the engine's and would
           have rejected every mission payload we now emit. */
        let ours = write_script(&sample(), "Qrow").unwrap();
        for script in [REAL, ours.as_str()] {
            assert_eq!(script.matches('{').count(), script.matches('}').count());
            let bytes = script.as_bytes();
            let mut at = 0;
            while let Some(offset) = script[at..].find('=') {
                let i = at + offset;
                let rest = &bytes[i + 1..];
                let end = rest
                    .iter()
                    .position(|c| matches!(c, b';' | b'}'))
                    .expect("a value with no terminator");
                assert_eq!(
                    rest[end], b';',
                    "unterminated value at {:?}",
                    &script[i.saturating_sub(24)..(i + 8).min(script.len())]
                );
                // Past this whole assignment, so padding inside the value is
                // not mistaken for the start of another one.
                at = i + 1 + end;
            }
        }
    }

    #[test]
    fn placed_units_travel_on_their_team() {
        // They used to be written to a side-car file that nothing read. Now
        // they are a team custom key, which is where Zero-K looks for them.
        let script = write_script(&sample(), "Qrow").unwrap();
        let team0 = script
            .split("[TEAM0]")
            .nth(1)
            .and_then(|s| s.split("[TEAM1]").next())
            .expect("no [TEAM0] section");
        let value = value_of(team0, "extrastartunits_1").expect("no units on team 0");
        let lua = String::from_utf8(decode_as_the_game_does(&value)).unwrap();
        assert!(lua.contains("armcom1"), "{lua}");
    }
}

// --------------------------------------------------------------- commands ---

/// Where a scenario's script is written before launching.
///
/// Deliberately not inside the Zero-K folder: a Steam install under
/// `Program Files` is not writable by a per-user process, and failing to launch
/// because of that would be a maddening bug to find.
fn script_path() -> std::path::PathBuf {
    std::env::temp_dir().join("splaunch").join("scenario_script.txt")
}

/// The extension a Splaunch scenario is saved under.
const SCENARIO_EXT: &str = "splaunch";

/// Read a scenario from disk.
///
/// A file from a newer Splaunch is refused by name rather than half-read: the
/// failure an author can act on is "this was written by a newer version", not a
/// missing objective they never notice.
pub fn from_json(text: &str) -> Result<Scenario, String> {
    let scenario: Scenario = serde_json::from_str(text)
        .map_err(|e| format!("that is not a Splaunch scenario: {e}"))?;
    if scenario.format_version > FORMAT_VERSION {
        return Err(format!(
            "This scenario was written by a newer Splaunch (format {}, this build reads {}).",
            scenario.format_version, FORMAT_VERSION
        ));
    }
    Ok(scenario)
}

pub fn to_json(scenario: &Scenario) -> Result<String, String> {
    serde_json::to_string_pretty(scenario).map_err(|e| format!("could not write it: {e}"))
}

/// Compile without launching, so the editor can show the script.
#[tauri::command]
pub fn spsc_script(scenario: Scenario, player: String) -> Result<String, String> {
    write_script(&scenario, &player)
}

/// What is wrong with it, for the count in the header.
///
/// `problems` itself is pure, so it can be tested without an install. The
/// checks that need to know about *this* machine are added here, on top.
#[tauri::command]
pub fn spsc_problems(game: tauri::State<'_, crate::launch::Game>, scenario: Scenario) -> Vec<String> {
    let mut out = problems(&scenario);
    out.extend(install_problems(game, &scenario));
    out
}

/// What is wrong with it that only this machine can answer.
///
/// Zero-K downloads maps on demand, so the catalogue lists 343 and an install
/// has a handful. Naming one that is not here fails at the engine with an error
/// about a missing archive, which is a poor way to learn you needed to play the
/// map once first.
///
/// Silent when there is no install or no map list: a machine that cannot answer
/// should not invent a complaint.
fn install_problems(game: tauri::State<'_, crate::launch::Game>, s: &Scenario) -> Vec<String> {
    let Some(root) = game.install_root() else { return Vec::new() };
    let installed = crate::game::installed_maps(&root);
    if installed.is_empty() || s.map.trim().is_empty() {
        return Vec::new();
    }
    if crate::game::map_is_installed(&installed, &s.map) {
        return Vec::new();
    }
    vec![format!(
        "The map {} is not installed. Play it once in the Zero-K lobby to download it.",
        s.map
    )]
}

/// The scenario Splaunch ships with.
///
/// Bundled into the binary rather than installed beside it: a portable build is
/// one `Splaunch.exe` that people unzip anywhere, and an example that only
/// works when a folder happens to be next to it is an example that mostly does
/// not work. Its `game` is filled in by the editor from whatever Zero-K is on
/// the machine.
#[tauri::command]
pub fn spsc_example() -> Result<Scenario, String> {
    from_json(include_str!("../../examples/first-contact.splaunch"))
}

/// Save a scenario, asking where.
///
/// Returns the path written, or `None` if the author closed the dialog - which
/// is not an error and should not be reported as one.
#[tauri::command]
pub fn spsc_save(app: tauri::AppHandle, scenario: Scenario) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let suggested = format!(
        "{}.{SCENARIO_EXT}",
        scenario.name.trim().replace(['/', '\\', ':'], "-")
    );
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Save scenario")
        .set_file_name(&suggested)
        .add_filter("Splaunch scenario", &[SCENARIO_EXT])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| format!("that is not a path this can write to: {e}"))?;
    std::fs::write(&path, to_json(&scenario)?)
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Open a scenario, asking which.
#[tauri::command]
pub fn spsc_open(app: tauri::AppHandle) -> Result<Option<Scenario>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Open scenario")
        .add_filter("Splaunch scenario", &[SCENARIO_EXT])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| format!("that is not a path this can read: {e}"))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    from_json(&text).map(Some)
}

/// Compile and launch the real game into it.
#[tauri::command]
pub fn spsc_test(
    app: tauri::AppHandle,
    game: tauri::State<'_, crate::launch::Game>,
    scenario: Scenario,
    player: String,
    engine: String,
) -> Result<u32, String> {
    let script = write_script(&scenario, &player)?;
    let script_path = script_path();
    if let Some(dir) = script_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    std::fs::write(&script_path, script)
        .map_err(|e| format!("could not write the script: {e}"))?;
    crate::launch::launch_script(app, game, &script_path, &engine)
}

