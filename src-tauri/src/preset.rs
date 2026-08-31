//! Coilbox presets: the battle a scenario is played in.
//!
//! Ported from `src/play/presets.ts` and `src/play/participants.ts` in
//! <https://github.com/tomjn/coilbox> at commit `20cbdf4d64e6`, MIT.
//!
//! Coilbox stacks four layers - a preset sets up a battle, a scenario puts
//! rules and events in it, a mission wraps a scenario with a briefing, a
//! campaign ties missions together - and its author's advice is to work from
//! the bottom up so that Coilbox can author each layer and be the reference
//! implementation. This is the bottom one, and it is also the layer their hub
//! already carries, because it is the only one with no media in it.
//!
//! It maps onto Splaunch almost exactly: a preset is the half of a `Scenario`
//! that is not scenario-specific - the map, the game, who is playing, and the
//! modoptions. What Splaunch adds on top is everything a preset has no room
//! for, which is the point of the layering.
//!
//! **What does not survive the round trip** is listed by [`ignored`] and shown
//! to the author rather than dropped in silence. Splaunch has nowhere to put a
//! faction, a handicap or a spectator, and refuses to invent one.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::container::{self, Container};
use crate::scenario::{Scenario, Team};

/// The container kind, and the payload version this build reads and writes.
pub const KIND: &str = "preset";
pub const KIND_VERSION: u32 = 1;

/// Their sentinel for "roll a concrete side at launch".
pub const RANDOM_SIDE: &str = "__random__";

/// An AI, as a preset names one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetAi {
    /// What the engine wants in `[AI] { ShortName = }` - "NullAI", "CircuitAI".
    pub short_name: String,
    /// "native" or "lua". Carried through; Splaunch launches either the same way.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One side of the battle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: String,
    /// "you" or "ai". The human is the one Splaunch writes as `[PLAYER0]`.
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<PresetAi>,
    /// The faction. Splaunch has no faction field, so this is preserved on the
    /// way out and reported as ignored on the way in.
    #[serde(default)]
    pub side: String,
    /// Red, green, blue, each 0..1 - the same space the start script uses.
    #[serde(default)]
    pub color: [f32; 3],
    pub ally_team: u32,
    #[serde(default)]
    pub spectator: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handicap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<u32>,
}

/// Unit and economy limits a preset can pin, so a shared battle replays faithfully.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Restrictions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_units: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advantage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub income_multiplier: Option<f64>,
}

/// A preset, as its payload.
///
/// Unknown fields are kept in `extra` rather than dropped: upstream adds fields
/// *without* bumping `kindVersion`, on the understanding that older readers
/// ignore them, so a round trip through Splaunch must not destroy what a newer
/// Coilbox put there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_used_at: String,
    #[serde(default)]
    pub participants: Vec<Participant>,
    #[serde(default)]
    pub game_name: String,
    #[serde(default)]
    pub map_name: String,
    #[serde(default)]
    pub start_pos_type: i32,
    #[serde(default)]
    pub mod_option_values: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrictions: Option<Restrictions>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A modoption value as a start script spells it.
///
/// The script is flat strings, and a preset's values are JSON. A bool is `1`
/// and `0` because that is what the engine and every Zero-K gadget read - a
/// literal `true` reaches Lua as the string "true", which is not falsy, so an
/// option turned *off* would read as on.
fn script_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(true) => "1".into(),
        serde_json::Value::Bool(false) => "0".into(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A colour as the start script's `RGBColor` wants it.
fn colour(rgb: [f32; 3]) -> String {
    let clamp = |c: f32| if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.0 };
    format!("{} {} {}", clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2]))
}

/// Whether a preset leaves nobody for the author to play.
///
/// A battle where you spectate is perfectly ordinary - the preset published on
/// their hub is exactly that, five participants of whom the human watches four
/// AIs fight. A *scenario* cannot be: `problems()` refuses one with no player,
/// because somebody has to be you. So the spectating human is kept and made to
/// play rather than dropped, and [`ignored`] says so.
fn spectating_human(preset: &Preset) -> bool {
    !preset.participants.iter().any(|p| !p.spectator && p.kind == "you")
        && preset.participants.iter().any(|p| p.spectator && p.kind == "you")
}

/// The teams a preset describes.
///
/// Spectators are dropped, because Splaunch writes one local player and every
/// other team as an AI and a spectator is neither - except the one case above.
/// Ids are assigned by position because the engine wants `[TEAM0]`, `[TEAM1]`
/// with no holes, and a preset's `team` field is a grouping, not an index.
pub fn teams_of(preset: &Preset) -> Vec<Team> {
    let rescue = spectating_human(preset);
    preset
        .participants
        .iter()
        .filter(|p| !p.spectator || (rescue && p.kind == "you"))
        .enumerate()
        .map(|(i, p)| Team {
            id: i as u32,
            ally: p.ally_team,
            ai: match (p.kind.as_str(), &p.ai) {
                ("you", _) => None,
                (_, Some(ai)) => Some(ai.short_name.clone()),
                // An "ai" with no AI named is still not the human.
                _ => Some("NullAI".into()),
            },
            colour: colour(p.color),
        })
        .collect()
}

/// What a preset carries that Splaunch cannot represent.
///
/// Returned so the editor can say so. Silently dropping a handicap would make a
/// shared battle play differently here than where it was made, which is the one
/// thing a preset exists to prevent.
pub fn ignored(preset: &Preset) -> Vec<String> {
    let mut out = Vec::new();
    let sides: Vec<&str> = preset
        .participants
        .iter()
        .filter(|p| !p.side.is_empty() && p.side != RANDOM_SIDE)
        .map(|p| p.side.as_str())
        .collect();
    if !sides.is_empty() {
        out.push(format!(
            "Factions are not kept: {}. Zero-K picks its own.",
            sides.join(", ")
        ));
    }
    if preset.participants.iter().any(|p| p.handicap.is_some_and(|h| h != 0.0)) {
        out.push("Handicaps are not kept - Splaunch writes no Handicap field.".into());
    }
    if spectating_human(preset) {
        out.push(
            "You were a spectator in that battle. A scenario needs a player, so you are              on a team now - move yourself if that is the wrong one."
                .into(),
        );
    }
    let dropped = preset
        .participants
        .iter()
        .filter(|p| p.spectator && !(spectating_human(preset) && p.kind == "you"))
        .count();
    if dropped > 0 {
        out.push(format!("{dropped} spectator(s) dropped - a scenario has none."));
    }
    if let Some(r) = &preset.restrictions {
        if r.disabled_units.as_ref().is_some_and(|u| !u.is_empty()) {
            out.push(
                "Disabled units are not applied yet - Splaunch writes NumRestrictions=0.".into(),
            );
        }
        if r.advantage.is_some() || r.income_multiplier.is_some() {
            out.push("Advantage and income multiplier are not applied yet.".into());
        }
    }
    if preset.start_pos_type != 2 {
        out.push(format!(
            "Start positions stay Splaunch's: a mission places its own, so startPosType {} is not used.",
            preset.start_pos_type
        ));
    }
    out
}

/// Lay a preset over a scenario, keeping everything the preset does not speak to.
///
/// The scenario's own name, units, objectives and briefing are untouched: a
/// preset is the battle, not what happens in it. Applying one to a scenario in
/// progress is meant to be a way to move it to another map and opponent.
pub fn apply(preset: &Preset, scenario: &mut Scenario) {
    scenario.map = preset.map_name.clone();
    if !preset.game_name.is_empty() {
        scenario.game = preset.game_name.clone();
    }
    let teams = teams_of(preset);
    if !teams.is_empty() {
        scenario.teams = teams;
    }
    scenario.mod_options = preset
        .mod_option_values
        .iter()
        .map(|(k, v)| (k.clone(), script_value(v)))
        .collect();
}

/// The battle half of a scenario, as a preset other tools can open.
///
/// `id` and the timestamps are the caller's to supply: this crate has no clock
/// and no randomness, and inventing either would make the output untestable.
pub fn from_scenario(scenario: &Scenario, id: &str, now: &str) -> Preset {
    let participants = scenario
        .teams
        .iter()
        .map(|t| {
            let rgb: Vec<f32> = t
                .colour
                .split_whitespace()
                .filter_map(|c| c.parse().ok())
                .collect();
            Participant {
                id: format!("team{}", t.id),
                kind: if t.ai.is_none() { "you".into() } else { "ai".into() },
                name: match &t.ai {
                    None => "You".into(),
                    Some(ai) => ai.clone(),
                },
                ai: t.ai.as_ref().map(|ai| PresetAi {
                    short_name: ai.clone(),
                    // Zero-K's shipped AIs are native; a Lua one would have to
                    // say so, and Splaunch does not record which it is.
                    kind: "native".into(),
                    name: None,
                }),
                side: RANDOM_SIDE.into(),
                color: [
                    rgb.first().copied().unwrap_or(0.0),
                    rgb.get(1).copied().unwrap_or(0.0),
                    rgb.get(2).copied().unwrap_or(0.0),
                ],
                ally_team: t.ally,
                spectator: false,
                handicap: None,
                team: None,
            }
        })
        .collect();

    Preset {
        id: id.to_string(),
        name: scenario.name.clone(),
        created_at: now.to_string(),
        last_used_at: now.to_string(),
        participants,
        game_name: scenario.game.clone(),
        map_name: scenario.map.clone(),
        // What `write_script` writes, rather than what a preset asked for.
        start_pos_type: 2,
        mod_option_values: scenario
            .mod_options
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect(),
        restrictions: None,
        extra: BTreeMap::new(),
    }
}

/// Read a preset out of a shared file or a pasted `cbz1.` code.
pub fn open(text: &str) -> Result<Preset, String> {
    Ok(container::open::<Preset>(text, KIND)?.payload)
}

/// Write one out in the form Coilbox reads.
pub fn to_json(preset: &Preset) -> Result<String, String> {
    container::to_json(&Container::new(KIND, KIND_VERSION, preset))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preset the way Coilbox writes one, envelope and all.
    const SHARED: &str = r#"{
      "format": "coilbox", "container": 1, "kind": "preset", "kindVersion": 1,
      "payload": {
        "id": "c6be936e", "name": "All That Simmers",
        "createdAt": "2026-08-01T00:00:00Z", "lastUsedAt": "2026-08-02T00:00:00Z",
        "gameName": "Zero-K v1.14.8.0", "mapName": "AlienDesert",
        "startPosType": 2,
        "modOptionValues": { "startmetal": 1000, "noelo": true, "hidden": false },
        "participants": [
          { "id": "p1", "kind": "you", "name": "You", "side": "__random__",
            "color": [0, 0, 1], "allyTeam": 0, "spectator": false },
          { "id": "p2", "kind": "ai", "name": "Enemy",
            "ai": { "shortName": "CircuitAI", "kind": "native" },
            "side": "Cortex", "color": [1, 0, 0], "allyTeam": 1, "spectator": false,
            "handicap": 20 },
          { "id": "p3", "kind": "you", "name": "Watcher", "side": "__random__",
            "color": [0, 1, 0], "allyTeam": 0, "spectator": true }
        ],
        "restrictions": { "disabledUnits": ["cloakraid"] }
      }
    }"#;

    #[test]
    fn a_coilbox_preset_becomes_a_battle() {
        let preset = open(SHARED).unwrap();
        assert_eq!(preset.map_name, "AlienDesert");

        let mut sc = crate::scenario::spsc_example().unwrap();
        apply(&preset, &mut sc);

        assert_eq!(sc.map, "AlienDesert");
        assert_eq!(sc.game, "Zero-K v1.14.8.0");
        // The spectator is not a team, and the two that are keep their sides.
        assert_eq!(sc.teams.len(), 2);
        assert_eq!(sc.teams[0].ai, None);
        assert_eq!(sc.teams[0].ally, 0);
        assert_eq!(sc.teams[1].ai.as_deref(), Some("CircuitAI"));
        assert_eq!(sc.teams[1].ally, 1);
        assert_eq!(sc.teams[1].colour, "1 0 0");
        // The scenario's own half is untouched by the battle it is played in.
        assert!(!sc.units.is_empty());
    }

    #[test]
    fn a_bool_modoption_reaches_the_script_as_one_and_zero() {
        /* A literal `true` arrives in Lua as the string "true", which is not
           falsy, so an option turned off would read as on. */
        let preset = open(SHARED).unwrap();
        let mut sc = crate::scenario::spsc_example().unwrap();
        apply(&preset, &mut sc);
        assert_eq!(sc.mod_options.get("noelo").map(String::as_str), Some("1"));
        assert_eq!(sc.mod_options.get("hidden").map(String::as_str), Some("0"));
        assert_eq!(sc.mod_options.get("startmetal").map(String::as_str), Some("1000"));
    }

    #[test]
    fn a_preset_cannot_disarm_the_mission_engine() {
        /* The one collision that matters: the last value of a repeated key is
           the one the engine keeps, so the mission's keys are written after a
           preset's and win. */
        let mut sc = crate::scenario::spsc_example().unwrap();
        sc.game = "Zero-K v1.14.8.0".into();
        sc.mod_options.insert("singleplayercampaignbattleid".into(), "hijacked".into());
        let script = crate::scenario::write_script(&sc, "Qrow").unwrap();
        let block = script.split("[MODOPTIONS]").nth(1).unwrap();
        let last = block
            .lines()
            .rfind(|l| l.trim().starts_with("singleplayercampaignbattleid="))
            .unwrap();
        assert!(last.contains("splaunch"), "{block}");
    }

    #[test]
    fn what_cannot_be_kept_is_said_rather_than_dropped() {
        let notes = ignored(&open(SHARED).unwrap());
        let all = notes.join(" | ");
        assert!(all.contains("Cortex"), "factions unreported: {all}");
        assert!(all.contains("Handicap"), "handicap unreported: {all}");
        assert!(all.contains("spectator"), "spectator unreported: {all}");
        assert!(all.contains("Disabled units"), "restrictions unreported: {all}");
    }

    #[test]
    fn a_field_this_build_never_heard_of_survives_the_round_trip() {
        /* Upstream adds fields without bumping `kindVersion`, on the
           understanding that older readers ignore them. Ignoring has to mean
           carrying, or Splaunch quietly deletes a newer Coilbox's work. */
        let text = SHARED.replace(
            r#""startPosType": 2,"#,
            r#""startPosType": 2, "somethingNewer": {"deep": [1,2,3]},"#,
        );
        let preset = open(&text).unwrap();
        assert!(preset.extra.contains_key("somethingNewer"), "{:?}", preset.extra);
        let out = to_json(&preset).unwrap();
        assert!(out.contains("somethingNewer"), "{out}");
        assert!(out.contains(r#""deep""#), "{out}");
    }

    /// The preset published on Coilbox's own hub, fetched 2026-08-31 from
    /// `coilbox-hub.vercel.app/i/c6be936e-58ed-4daa-941e-800317876663`.
    /// Asserting against the real thing rather than against a fixture written
    /// to match the code, which is the whole point of keeping one.
    const REAL: &str = include_str!("fixtures/coilbox-preset.json");

    #[test]
    fn the_preset_on_their_hub_opens() {
        let preset = open(REAL).unwrap();
        assert_eq!(preset.map_name, "All That Simmers v1.1.1");
        assert_eq!(preset.game_name, "SplinterFaction 0.1.78");
        assert_eq!(preset.participants.len(), 5);

        // `game` is an additive field upstream never bumped a version for.
        assert!(preset.extra.contains_key("game"), "{:?}", preset.extra.keys());
        assert!(to_json(&preset).unwrap().contains("SplinterFaction"));

        let mut sc = crate::scenario::spsc_example().unwrap();
        apply(&preset, &mut sc);
        assert_eq!(sc.map, "All That Simmers v1.1.1");
        // Four Lua AIs across two allyteams, and their names reach the script.
        assert_eq!(sc.teams.iter().filter(|t| t.ai.is_some()).count(), 4);
        assert!(sc.teams.iter().any(|t| t.ai.as_deref() == Some("SurvivalAI")));
        assert_eq!(sc.mod_options.get("deathmode").map(String::as_str), Some("com"));
    }

    #[test]
    fn a_battle_you_only_watched_still_gives_you_a_team() {
        /* Their published preset has the human spectating five-way while four
           AIs fight. That is a fine battle and an impossible scenario, because
           `problems()` refuses one with nobody to be. */
        let preset = open(REAL).unwrap();
        assert!(preset.participants[0].spectator && preset.participants[0].kind == "you");

        let mut sc = crate::scenario::spsc_example().unwrap();
        sc.game = "Zero-K v1.14.8.0".into();
        apply(&preset, &mut sc);

        assert_eq!(sc.teams.iter().filter(|t| t.ai.is_none()).count(), 1,
            "exactly one team has to be the player: {:?}", sc.teams);
        assert!(
            !crate::scenario::problems(&sc).iter().any(|p| p.contains("somebody has to be you")),
            "{:?}", crate::scenario::problems(&sc)
        );
        assert!(ignored(&preset).iter().any(|n| n.contains("spectator in that battle")),
            "{:?}", ignored(&preset));
    }

    #[test]
    fn a_scenario_exports_as_a_preset_coilbox_would_read() {
        let mut sc = crate::scenario::spsc_example().unwrap();
        sc.game = "Zero-K v1.14.8.0".into();
        let json = to_json(&from_scenario(&sc, "abc123", "2026-08-31T00:00:00Z")).unwrap();

        let id = container::identify(&json).unwrap();
        assert_eq!(id.kind, KIND);
        assert_eq!(id.kind_version, KIND_VERSION);
        assert_eq!(id.compatibility, container::Compatibility::Supported);

        let back = open(&json).unwrap();
        assert_eq!(back.map_name, sc.map);
        assert_eq!(back.participants.len(), sc.teams.len());
        assert_eq!(back.participants[0].kind, "you");
        assert!(back.participants.iter().any(|p| p.kind == "ai"));
    }
}

// --------------------------------------------------------------- commands ---

/// A preset laid over a scenario, and what could not come with it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub scenario: Scenario,
    /// Shown to the author. See [`ignored`] for why this is not silent.
    pub ignored: Vec<String>,
    pub name: String,
}

/// Open a Coilbox preset and lay it over the scenario in hand.
///
/// `text` is for a pasted `cbz1.` code; without one this opens a file picker.
/// `Ok(None)` means the author closed the dialog, which is not an error.
#[tauri::command]
pub fn spsc_import_preset(
    app: tauri::AppHandle,
    scenario: Scenario,
    text: Option<String>,
) -> Result<Option<Applied>, String> {
    use tauri_plugin_dialog::DialogExt;
    let text = match text.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => {
            let Some(path) = app
                .dialog()
                .file()
                .set_title("Open Coilbox preset")
                .add_filter("Coilbox preset", &["json"])
                .blocking_pick_file()
            else {
                return Ok(None);
            };
            let path = path
                .into_path()
                .map_err(|e| format!("that is not a path this can read: {e}"))?;
            std::fs::read_to_string(&path)
                .map_err(|e| format!("could not read {}: {e}", path.display()))?
        }
    };
    let preset = open(&text)?;
    let mut scenario = scenario;
    apply(&preset, &mut scenario);
    Ok(Some(Applied {
        ignored: ignored(&preset),
        name: preset.name.clone(),
        scenario,
    }))
}

/// Write the battle half of this scenario out as a preset.
#[tauri::command]
pub fn spsc_export_preset(
    app: tauri::AppHandle,
    scenario: Scenario,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    if scenario.map.trim().is_empty() {
        return Err("Choose a map first - a preset is a battle, and a battle needs one.".into());
    }
    /* Seconds since the epoch, for both the id and the timestamps. Coilbox
       re-identifies an imported preset anyway, so this only has to be unique
       here; the pure half of this module takes them as arguments so it stays
       testable without a clock. */
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("splaunch-{}-{stamp}", crate::campaign::slug(&scenario.name));
    // RFC 3339 is what their timestamps look like; without a date library the
    // honest thing is the epoch instant rather than a wrong-looking local date.
    let now = format!("1970-01-01T00:00:00Z+{stamp}s");
    let json = to_json(&from_scenario(&scenario, &id, &now))?;

    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export Coilbox preset")
        .set_file_name(format!("{id}.json"))
        .add_filter("Coilbox preset", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| format!("that is not a path this can write to: {e}"))?;
    std::fs::write(&path, json.as_bytes())
        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(Some(path.display().to_string()))
}
