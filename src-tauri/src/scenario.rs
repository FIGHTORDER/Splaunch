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
    /// 0-3, quarter turns. Absent is written as 0 rather than left out, which
    /// is not the same thing: the gadget cannot place a unit without one.
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
    /// 0-3, written as 0 when absent for the same reason as a unit's.
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
    /// The map's width in elmos - the x axis.
    #[serde(default = "default_map_elmos")]
    pub map_elmos: u32,
    /// The map's depth in elmos - the z axis - where it differs from the width.
    ///
    /// Absent means square, which is what a file written before this field
    /// existed meant. Splaunch carried one figure for both axes and drew every
    /// map as a square; 145 of the catalogue's 343 maps are not square, so on
    /// those every unit went in at the wrong depth. The shipped example was one
    /// of them - it names Comet Catcher Redux, which is 12 x 16.
    ///
    /// Left out of the file when it is not needed, so a square scenario still
    /// reads correctly in a Splaunch that predates this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_elmos_z: Option<u32>,
    /// Modoptions the battle carries besides the mission's own.
    ///
    /// Where a Coilbox preset's `modOptionValues` land: a preset sets up a
    /// battle, and a battle is partly its modoptions. Written before the
    /// mission's keys so that a preset cannot overwrite the mission engine -
    /// the last value of a repeated key is the one the engine keeps.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub mod_options: std::collections::BTreeMap<String, String>,
    /// Labels on the map, shown from the start.
    #[serde(default)]
    pub markers: Vec<Marker>,
    /// 1 easy, 2 normal, 3 hard. Gates the per-unit `difficultyAt*` fields, so
    /// one scenario can be three.
    #[serde(default = "default_difficulty")]
    pub difficulty: u32,
}

impl Scenario {
    /// The map's depth in elmos, filling in the square a file may have meant.
    pub fn depth_elmos(&self) -> u32 {
        self.map_elmos_z.unwrap_or(self.map_elmos)
    }
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
    let across = s.map_elmos as f32;
    let down = s.depth_elmos() as f32;
    if let Some(stray) = s
        .units
        .iter()
        .find(|u| u.x < 0.0 || u.z < 0.0 || u.x > across || u.z > down)
    {
        out.push(format!(
            "A {} sits outside the map, at {}, {}. The map is {} by {} elmos.",
            stray.unit,
            stray.x as i64,
            stray.z as i64,
            s.map_elmos,
            s.depth_elmos()
        ));
    }
    for goal in &s.goals {
        if goal.description.trim().is_empty() {
            out.push("An objective has no description, so the player cannot read it.".into());
        }
    }
    out
}

/// What an author should know but that must not stop a launch.
///
/// Separate from `problems` because that list is fatal - `write_script` refuses
/// on its first entry - and none of these are worth refusing over. They are the
/// things that are silently *decided* for an author rather than wrong.
pub fn warnings(s: &Scenario) -> Vec<String> {
    let mut out = Vec::new();
    /* Zero-K spawns every team a commander whether or not the author placed
       one, and puts it at the team's start position. A placed commander says
       where that is; without one the position is a guess. Unmentioned, the
       guess reads as the game ignoring the scenario - which is exactly how it
       looked the first time a mission was launched with no commander on it. */
    for t in &s.teams {
        if commander_index(s, t.id).is_some() {
            continue;
        }
        let has_units = s.units.iter().any(|u| !u.neutral && u.team == t.id);
        let (x, z) = team_start(s, t.id);
        let whose = if t.ai.is_none() { "your team" } else { "team" };
        out.push(format!(
            "Zero-K gives {whose} {} a commander whether or not you place one. \
             With none placed it will start at {}, {} - {}. Place a commander \
             to say where.",
            t.id,
            x as i64,
            z as i64,
            if has_units { "the middle of its units" } else { "the middle of the map" },
        ));
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

    /* Makes Zero-K read each team's `start_x`/`start_z` instead of the start
       box (`start_unit_setup.lua:38`, `:255-259`). Without it the commanders
       spawn from the box, which this writes as a single point in the corner of
       the map, and no amount of placing moves them. */
    out.push(("fixedstartpos".into(), "1".into()));

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
            // As above, and one step worse: the gadget computes
            // `facing * FACING_TO_HEADING`, so an absent one is arithmetic on
            // nil rather than a rejected argument.
            entry.set("facing", ck::n(feature.facing.unwrap_or(0)));
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
    /* Always sent, even with nothing to say. The briefing widget does
       `caption = "Planet " .. planetInformation.name` with no guard
       (`mission_galaxy_battle_handler.lua:327`) over a table that defaults to
       empty, so an absent key is a nil concatenation - a Lua error during
       `widget:Initialize`, which takes the whole widget down. That widget also
       draws the objectives panel, so a scenario with no briefing lost its
       objectives display too, and looked from the outside like objectives were
       never evaluated. */
    let notes: Vec<&String> = s.objectives.iter().filter(|o| !o.trim().is_empty()).collect();
    let briefing = s.briefing.as_deref().map(str::trim).filter(|t| !t.is_empty());
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
    /* Facing is always written, and it is the exception to the rule below.
       The gadget hands it straight to `Spring.CreateUnit` as a positional
       argument, so leaving it out is not "let the game choose": it is
       `CreateUnit(): bad facing parameter`, and the call that raises it is the
       one placing every unit in the scenario. Measured on a real engine, where
       the shipped example placed nothing at all for this reason. */
    entry.set("facing", ck::n(unit.facing.unwrap_or(0)));
    // The rest are written only when set. The gadget branches on the field
    // being present, so a defaulted zero is a different instruction from
    // silence - `buildProgress = 0` is an unbuilt husk, not a finished unit.
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

/// The placed unit that stands for a team's commander, if the author placed one.
///
/// Zero-K spawns each team's commander itself and puts it at the team's
/// `start_x`/`start_z` (`start_unit_setup.lua:255-259`); `extrastartunits` are
/// what arrives *besides* it. So a placed commander is not a unit to spawn - it
/// is where the team starts, and spawning it as well gave the team two, one of
/// them wherever the start box happened to point.
fn commander_index(s: &Scenario, team: u32) -> Option<usize> {
    s.units
        .iter()
        .position(|u| !u.neutral && u.team == team && crate::game::is_commander(&u.unit))
}

/// Where a team's commander arrives.
///
/// The placed commander if there is one. Failing that the middle of what the
/// team does have, because Zero-K spawns a commander whether or not the author
/// placed one, and next to their own units is the only defensible guess. The
/// middle of the map when the team has nothing at all - anywhere is arbitrary,
/// but a corner is arbitrary *and* looks like a bug, which is how this was
/// found.
fn team_start(s: &Scenario, team: u32) -> (f32, f32) {
    if let Some(i) = commander_index(s, team) {
        return (s.units[i].x, s.units[i].z);
    }
    let mine: Vec<&Placed> = s.units.iter().filter(|u| !u.neutral && u.team == team).collect();
    if mine.is_empty() {
        return (s.map_elmos as f32 / 2.0, s.depth_elmos() as f32 / 2.0);
    }
    let n = mine.len() as f32;
    (
        mine.iter().map(|u| u.x).sum::<f32>() / n,
        mine.iter().map(|u| u.z).sum::<f32>() / n,
    )
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
    /* The battle's own, from a preset. First, so the mission's keys below
       win any collision - losing `singleplayercampaignbattleid` to a stray
       preset value would disarm the mission engine entirely. */
    for (name, value) in &s.mod_options {
        key(&mut out, "\t\t", &escape(name), escape(value));
    }
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
        /* Where Zero-K puts this team's commander. Read off the team's custom
           keys, and only when `fixedstartpos` is set - without both, the engine
           falls back to the start box, and the box below is a single point in
           the corner of the map. That is why every commander arrived in a
           corner however carefully it had been placed. */
        let (sx, sz) = team_start(s, t.id);
        key(&mut out, "\t\t", "start_x", sx.round() as i64);
        key(&mut out, "\t\t", "start_z", sz.round() as i64);

        // Placed units ride on the team that owns them, which is how Zero-K
        // knows whose they are without a field saying so. The commander is not
        // among them: it is the start position, and the game spawns it.
        let commander = commander_index(s, t.id);
        let mine = s
            .units
            .iter()
            .enumerate()
            .filter(|(i, u)| !u.neutral && u.team == t.id && Some(*i) != commander)
            .map(|(_, u)| u);
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

           It is not an empty box but a single point in the top-right corner -
           top and bottom both 0, left and right both 1 - and until
           `fixedstartpos` was sent that is exactly where every commander
           spawned, however carefully it had been placed. It stays because
           `fixedstartpos` bypasses it for spawning and it was copied from a
           mission that runs; a team's real start is `start_x`/`start_z`. */
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
            /* A commander and something else. The commander is the team's
               start position rather than one of its `extrastartunits`, so a
               fixture with only a commander exercises none of the spawn path. */
            units: vec![
                Placed {
                    unit: "armcom1".into(),
                    team: 0,
                    x: 512.0,
                    z: 512.0,
                    ..Default::default()
                },
                Placed {
                    unit: "cloakraid".into(),
                    team: 0,
                    x: 640.0,
                    z: 640.0,
                    ..Default::default()
                },
            ],
            objectives: vec!["Destroy the enemy commander".into()],
            goals: vec![],
            features: vec![],
            briefing: None,
            defeat: vec![],
            format_version: FORMAT_VERSION,
            map_elmos: DEFAULT_MAP_ELMOS,
            map_elmos_z: None,
            mod_options: Default::default(),
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
    fn a_briefing_is_sent_whether_or_not_there_is_something_to_read() {
        /* This used to assert the opposite, and the opposite was the bug: an
           absent key is a nil concatenation in the widget that draws the
           briefing *and* the objectives, so a scenario with nothing to say
           lost both panels. Sent always, empty if need be. */
        let mut bare = sample();
        bare.objectives.clear();
        bare.briefing = None;
        let script = write_script(&bare, "Qrow").unwrap();
        assert!(script.contains("planetmissioninformationtext"), "{script}");

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
    fn the_briefing_key_is_sent_even_with_nothing_to_say() {
        /* An absent `planetmissioninformationtext` is a nil concatenation in
           `mission_galaxy_battle_handler.lua:327`, which kills the widget that
           draws both the briefing and the objectives panel. A scenario with no
           briefing must still send a name. */
        let mut sc = sample();
        sc.briefing = None;
        sc.objectives = vec![];

        let script = write_script(&sc, "Qrow").unwrap();
        let lua = modoption_lua(&script, "planetmissioninformationtext");
        assert!(lua.contains("name="), "no name in {lua}");
        assert!(lua.contains("description="), "no description in {lua}");
    }

    #[test]
    fn the_two_map_axes_are_checked_separately() {
        /* A square check on a map that is not square is wrong in both
           directions at once: it waves through a unit past the short edge and
           refuses one that is comfortably inside the long one. Comet Catcher
           Redux, which the shipped example names, is 12 x 16. */
        let mut sc = sample();
        sc.map_elmos = 12 * 512;
        sc.map_elmos_z = Some(16 * 512);

        sc.units[0].x = 7000.0; // past the short edge, well inside a square check
        sc.units[0].z = 100.0;
        assert!(
            problems(&sc).iter().any(|p| p.contains("outside the map")),
            "a unit past the width was allowed: {:?}",
            problems(&sc)
        );

        sc.units[0].x = 100.0;
        sc.units[0].z = 7000.0; // inside the depth, outside a square check
        assert!(
            !problems(&sc).iter().any(|p| p.contains("outside the map")),
            "a unit inside the depth was refused: {:?}",
            problems(&sc)
        );
    }

    #[test]
    fn a_square_scenario_writes_no_depth_at_all() {
        /* The depth is left out when it says nothing, so a square scenario
           saved by this version still opens in one that predates the field. */
        let sc = sample();
        let json = serde_json::to_string(&sc).unwrap();
        assert!(!json.contains("mapElmosZ"), "{json}");

        let mut tall = sample();
        tall.map_elmos_z = Some(8192);
        let json = serde_json::to_string(&tall).unwrap();
        assert!(json.contains("\"mapElmosZ\":8192"), "{json}");
        assert_eq!(from_json(&json).unwrap().depth_elmos(), 8192);
    }

    #[test]
    fn optional_unit_fields_are_omitted_rather_than_defaulted() {
        /* The gadget branches on a field being present. `buildProgress = 0` is
           an unbuilt husk, not a finished unit, so writing a default would
           change what spawns.

           Facing is the exception and is always present: it is a positional
           argument to `CreateUnit` rather than a field the gadget tests for. */
        let plain = to_lua(&placed_fields(&Placed {
            unit: "cloakraid".into(),
            team: 0,
            x: 1.0,
            z: 2.0,
            ..Default::default()
        }));
        assert_eq!(plain, "{name=\"cloakraid\",x=1,z=2,facing=0,}");

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
        sc.units[1].patrol = vec![[100.0, 200.0], [800.0, 900.0]];
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
        sc.units[1].patrol = vec![[10.0, 20.0], [30.0, 40.0]];
        sc.units[1].self_patrol = true;
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team0, "extrastartunits_1").unwrap())).unwrap();
        assert!(lua.contains("patrolRoute"), "{lua}");
        assert!(!lua.contains("selfPatrol"), "{lua}");

        // A one-point "route" is not a route, so it falls through.
        sc.units[1].patrol = vec![[10.0, 20.0]];
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
        sc.units[1].difficulty_at_least = Some(3);
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

    /// The shipped example, compiled against a real install and written out.
    ///
    /// Every other test here compiles against fixtures, which is why they all
    /// passed while `GameType` was naming a racing mod and `Mapname` was a
    /// string the engine has never indexed. This one asks the machine.
    ///
    /// Ignored, because CI has no Zero-K:
    ///
    /// ```text
    /// SPLAUNCH_TEST_ZK_ROOT=... SPLAUNCH_TEST_SCRIPT=out.txt \
    ///   cargo test --lib -- --ignored --nocapture
    /// ```
    ///
    /// `SPLAUNCH_TEST_MAP` overrides the example's map, for a machine that has
    /// Zero-K but not that one.
    #[test]
    #[ignore = "needs a Zero-K install in SPLAUNCH_TEST_ZK_ROOT"]
    fn the_shipped_example_compiles_against_a_real_install() {
        let root = std::path::PathBuf::from(
            std::env::var("SPLAUNCH_TEST_ZK_ROOT")
                .expect("set SPLAUNCH_TEST_ZK_ROOT to a Zero-K data directory"),
        );
        let mut s = from_json(include_str!("../../examples/first-contact.splaunch"))
            .expect("the shipped example does not parse");

        s.game = crate::game::base_game(&root).expect("no game in that install").name;
        assert!(
            s.game.to_ascii_lowercase().starts_with("zero-k"),
            "the game to launch came out as {:?}",
            s.game
        );

        if let Ok(map) = std::env::var("SPLAUNCH_TEST_MAP") {
            s.map = map;
        }
        let installed = crate::game::installed_maps(&root);
        s.map = crate::game::resolve_map(&installed, &s.map).unwrap_or_else(|| {
            panic!("the map {:?} is not installed. Installed: {installed:?}", s.map)
        });

        let problems = problems(&s);
        assert!(problems.is_empty(), "{problems:#?}");

        let script = write_script(&s, "Tester").expect("the example does not compile");
        for line in script.lines() {
            let line = line.trim();
            if line.starts_with("Mapname") || line.starts_with("GameType") {
                println!("{line}");
            }
        }
        if let Ok(path) = std::env::var("SPLAUNCH_TEST_SCRIPT") {
            std::fs::write(&path, &script).expect("could not write the script");
            println!("wrote {path} ({} bytes)", script.len());
        }
    }

    #[test]
    fn a_unit_with_no_facing_still_reaches_the_game_with_one() {
        /* Measured against a real engine. Without this the campaign gadget
           raises `CreateUnit(): bad facing parameter` on the first unit and the
           scenario runs on an empty map, with nothing in the log to say that
           the units were the thing that failed. */
        let mut sc = sample();
        sc.units[0].facing = None;
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap();
        let value = value_of(team0, "extrastartunits_1").expect("no units on team 0");
        let placed = String::from_utf8(decode_as_the_game_does(&value)).unwrap();
        assert!(placed.contains("facing=0"), "{placed}");
    }

    #[test]
    fn a_feature_with_no_facing_still_reaches_the_game_with_one() {
        let mut sc = sample();
        sc.features = vec![Feature {
            name: "cloakraid_dead".into(),
            x: 100.0,
            z: 200.0,
            facing: None,
        }];
        let script = write_script(&sc, "Qrow").unwrap();
        let spawned = modoption_lua(&script, "featurestospawn");
        assert!(spawned.contains("facing=0"), "{spawned}");
    }

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
    fn the_example_carries_the_real_shape_of_its_map() {
        /* Comet Catcher Redux is 12 x 16, and the example was drawn against one
           figure for both axes - so the editor showed a square, cropped to the
           middle of a map half again as deep as it was drawn. The example is
           the first thing anybody opens, so it is the last place to leave that. */
        let sc = from_json(EXAMPLE).expect("the example does not parse");
        assert_eq!(sc.map_elmos, 12 * 512);
        assert_eq!(sc.depth_elmos(), 16 * 512);
        assert!(
            !problems(&sc).iter().any(|p| p.contains("outside the map")),
            "{:?}",
            problems(&sc)
        );
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
        // A file from before the depth existed meant a square map, and says so.
        assert_eq!(sc.map_elmos_z, None);
        assert_eq!(sc.depth_elmos(), DEFAULT_MAP_ELMOS);
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
            map_elmos: DEFAULT_MAP_ELMOS, map_elmos_z: None,
            mod_options: Default::default(), markers: vec![],
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
        // Not the commander: see the test below for where that one goes.
        assert!(lua.contains("cloakraid"), "{lua}");
    }

    #[test]
    fn a_placed_commander_is_where_the_team_starts_not_a_unit_to_spawn() {
        /* Zero-K spawns each team's commander itself, at the team's
           `start_x`/`start_z`, and `extrastartunits` are what arrives besides
           it. Sending a commander as a start unit gave the team two of them -
           the placed one, and the game's own in whatever corner the start box
           named. Every scenario launched before this had that. */
        let script = write_script(&sample(), "Qrow").unwrap();

        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        assert_eq!(value_of(block, "fixedstartpos").as_deref(), Some("1"),
            "without this the engine ignores start_x and uses the start box: {block}");

        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        assert_eq!(value_of(team0, "start_x").as_deref(), Some("512"), "{team0}");
        assert_eq!(value_of(team0, "start_z").as_deref(), Some("512"), "{team0}");

        let lua = String::from_utf8(
            decode_as_the_game_does(&value_of(team0, "extrastartunits_1").unwrap())).unwrap();
        assert!(!lua.contains("armcom1"), "the commander was spawned as well: {lua}");
    }

    #[test]
    fn a_team_with_no_commander_starts_among_its_own_units() {
        /* Zero-K spawns a commander whether or not the author placed one, so
           the position still has to say something. Next to the team's own units
           beats the corner of the map, which is what an unset start meant. */
        let mut sc = sample();
        sc.units = vec![
            Placed { unit: "cloakraid".into(), team: 0, x: 1000.0, z: 2000.0, ..Default::default() },
            Placed { unit: "cloakraid".into(), team: 0, x: 3000.0, z: 4000.0, ..Default::default() },
        ];
        let script = write_script(&sc, "Qrow").unwrap();
        let team0 = script.split("[TEAM0]").nth(1).unwrap().split("[TEAM1]").next().unwrap();
        assert_eq!(value_of(team0, "start_x").as_deref(), Some("2000"), "{team0}");
        assert_eq!(value_of(team0, "start_z").as_deref(), Some("3000"), "{team0}");

        /* And a team with nothing at all lands in the middle, not in a corner.
           Team 1 of the sample has no units of its own. */
        let script = write_script(&sample(), "Qrow").unwrap();
        let team1 = script.split("[TEAM1]").nth(1).unwrap().split("[ALLYTEAM").next().unwrap();
        assert_eq!(value_of(team1, "start_x").as_deref(), Some("2048"), "{team1}");
        assert_eq!(value_of(team1, "start_z").as_deref(), Some("2048"), "{team1}");
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

/// What an author should know, for the panel under the blockers.
#[tauri::command]
pub fn spsc_warnings(scenario: Scenario) -> Vec<String> {
    warnings(&scenario)
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
    mut scenario: Scenario,
    player: String,
    engine: String,
) -> Result<u32, String> {
    /* `Mapname` has to be the archive's own name, and a scenario carries the
       one its author typed. Those differ more often than not, because the
       archive carries a version the author had no reason to write down:
       "Comet Catcher Redux" against "Comet Catcher Redux v3.1". Left alone the
       engine stops with an error about a missing map, which reads like the map
       is not installed when it is.

       Only a name that resolves is replaced. One that does not is passed
       through as written, so the engine's own error is what the author sees
       rather than a guess of ours. */
    if let Some(root) = game.install_root() {
        let installed = crate::game::installed_maps(&root);
        if let Some(exact) = crate::game::resolve_map(&installed, &scenario.map) {
            scenario.map = exact;
        }
    }
    let script = write_script(&scenario, &player)?;
    let script_path = script_path();
    if let Some(dir) = script_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    std::fs::write(&script_path, script)
        .map_err(|e| format!("could not write the script: {e}"))?;
    crate::launch::launch_script(app, game, &script_path, &engine)
}

