//! Packaging missions into a campaign that Shiro can install and run.
//!
//! A campaign is a zip: a `campaign.json` naming its missions and the order to
//! play them in, and for each mission a compiled start script beside the
//! scenario it came from.
//!
//! The compiled script is the point. A start script has exactly three fields
//! that cannot travel between machines - the map's archive name, the local
//! Zero-K's archive name, and the player - and everything else in it, including
//! every base64 payload carrying units, objectives, features and the briefing,
//! is the same on every machine. So a mission ships compiled, with those three
//! written as markers, and whoever runs it substitutes three strings.
//!
//! That is why Shiro needs no copy of this crate's scenario compiler. Two
//! implementations of a format whose encoder has to work around two faults in
//! the game's own decoder would drift, and the drift would show up as a
//! mission that silently places nothing.
//!
//! The scenario travels too, so a campaign can be opened and changed in the
//! editor. Nothing reads it at run time.

use serde::{Deserialize, Serialize};

use crate::scenario::{write_script, Scenario};

/// The map's archive name, which carries a version the author never typed.
pub const HOLE_MAP: &str = "__SHIRO_MAP__";
/// The local Zero-K, which differs between a Steam install and a rapid one.
pub const HOLE_GAME: &str = "__SHIRO_GAME__";
/// Whoever is playing.
pub const HOLE_PLAYER: &str = "__SHIRO_PLAYER__";

pub const CAMPAIGN_EXT: &str = "shirocamp";

const fn current_format() -> u32 {
    1
}

/// One mission, as the campaign lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignMission {
    /// Stable, and the name of its files inside the zip.
    pub id: String,
    pub name: String,
    /// Repeated outside the script so a loader can say "this campaign needs two
    /// maps you do not have" before launching rather than at the whistle.
    pub map: String,
    /// Mission ids that must be finished first. A linear campaign names its
    /// predecessor; a branching one names several. Empty means it is open from
    /// the start, and a campaign with no open mission is refused.
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// A campaign, as `campaign.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    #[serde(default = "current_format")]
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// The Zero-K these were compiled against.
    ///
    /// Unit names are stable and the campaign gadget ignores types it cannot
    /// resolve, which fails quietly: a mission built against a version that
    /// renamed something places nothing and says nothing. Recording the version
    /// lets a loader say so instead of leaving the player on an empty map.
    #[serde(default)]
    pub built_against: Option<String>,
    pub missions: Vec<CampaignMission>,
}

/// A mission compiled for a machine we do not have.
///
/// The scenario's own map and game are replaced by markers before compiling
/// rather than after, so the substitution goes through the same escaping as any
/// other value and cannot produce a script this crate would not have written.
pub fn mission_template(scenario: &Scenario) -> Result<String, String> {
    let mut portable = scenario.clone();
    portable.map = HOLE_MAP.to_string();
    portable.game = HOLE_GAME.to_string();
    let script = write_script(&portable, HOLE_PLAYER)?;
    verify_payloads(&script)?;
    Ok(script)
}

/// Fill a template in for this machine.
///
/// Test-only, and deliberately: filling one in is the loader's job, not the
/// editor's. It is here because the round trip is the property worth checking -
/// a template that cannot be filled back into exactly what this crate would
/// have compiled is not a mission, and that check belongs beside the code that
/// writes the holes rather than in the other repository.
#[cfg(test)]
fn fill(template: &str, map: &str, game: &str, player: &str) -> String {
    template
        .replace(HOLE_MAP, map)
        .replace(HOLE_GAME, game)
        .replace(HOLE_PLAYER, player)
}

/// Every payload in the script, checked for the byte that would destroy it.
///
/// Zero-K rewrites `_` to `=` before decoding, and its own alphabet maps 63 to
/// `_`, so a payload carrying a literal `_` is destroyed on the way in.
/// `customkey` escapes the bytes that would produce one, and a sweep of every
/// byte value at every alignment says it works. This checks anyway, because a
/// campaign is compiled once and run by everybody who downloads it: a payload
/// that does not survive is not one machine's bad luck, it is broken for all of
/// them, and the symptom is a mission that starts on an empty map with nothing
/// in the log.
fn verify_payloads(script: &str) -> Result<(), String> {
    for (key, value) in payloads(script) {
        if !is_encoded(&key) {
            continue;
        }
        /* `_` is the whole failure, and it is checked for directly rather than
           inferred from a decode. Zero-K rewrites `_` to `=` before decoding
           and `=` is absent from its alphabet, so everything from there on is
           dropped - and a truncated Lua literal does not parse, which loses the
           entire payload rather than one field of it.

           Decoding and testing for emptiness does not catch that: a `_` past
           the first few characters leaves a non-empty prefix, which passes
           while the mission is still broken. */
        if let Some(at) = value.find('_') {
            return Err(format!(
                "the {key} payload carries a '_' at byte {at}, which Zero-K's \
                 decoder reads as end-of-data - this mission would start with \
                 nothing on the map"
            ));
        }
    }
    Ok(())
}

/// Whether a start-script key carries one of our base64 payloads.
///
/// By key rather than by the shape of the value, which is what it used to be.
/// Two things break the shape test. Our alphabet is URL-safe, so a payload can
/// legitimately contain `-` and `_` - and `_` is not alphanumeric, so the old
/// filter skipped every payload carrying the one byte the check exists to find.
/// And a template's `Mapname` is `__SHIRO_MAP__`, which is all underscores and
/// would fail a check it has no business being subject to.
fn is_encoded(key: &str) -> bool {
    const KEYS: &[&str] = &[
        "bonusobjectiveconfig",
        "objectiveconfig",
        "defeatconditionconfig",
        "featurestospawn",
        "planetmissioninformationtext",
        "planetmissionmapmarkers",
        "initalterraform",
    ];
    let key = key.trim().to_ascii_lowercase();
    KEYS.contains(&key.as_str())
        || key.starts_with("extrastartunits_")
        || key.starts_with("neutralstartunits_")
}

/// `key=value;` pairs inside the script's `[MODOPTIONS]` block and its teams.
fn payloads(script: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in script.lines() {
        let line = line.trim().trim_end_matches(';');
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        out.push((key.to_string(), value.to_string()));
    }
    out
}

/// A file name from a mission's name.
///
/// Ids end up as paths inside the zip and are the only part of a campaign that
/// a loader turns back into a filename, so they hold to what is safe on every
/// platform rather than to what looks tidy.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "mission".to_string()
    } else {
        trimmed
    }
}

/// What is wrong with a campaign, before it is written.
pub fn problems(campaign: &Campaign, missions: &[(String, Scenario)]) -> Vec<String> {
    let mut out = Vec::new();
    if campaign.name.trim().is_empty() {
        out.push("The campaign has no name.".into());
    }
    if campaign.missions.is_empty() {
        out.push("The campaign has no missions.".into());
    }
    let ids: Vec<&str> = campaign.missions.iter().map(|m| m.id.as_str()).collect();
    for (i, id) in ids.iter().enumerate() {
        if ids[..i].contains(id) {
            out.push(format!("Two missions share the id {id}."));
        }
        if !missions.iter().any(|(mid, _)| mid == id) {
            out.push(format!("The mission {id} is listed but has no scenario."));
        }
    }
    for m in &campaign.missions {
        for need in &m.requires {
            if !ids.contains(&need.as_str()) {
                out.push(format!(
                    "{} needs {need}, which is not a mission in this campaign.",
                    m.name
                ));
            }
            if need == &m.id {
                out.push(format!("{} requires itself.", m.name));
            }
        }
    }
    /* A campaign whose every mission is locked can never be started. Cycles
       produce exactly this, which is why it is checked here rather than left
       for the loader to discover as an empty list. */
    if !campaign.missions.is_empty() && campaign.missions.iter().all(|m| !m.requires.is_empty()) {
        out.push("Every mission requires another, so none of them can be played first.".into());
    }
    out
}

/// Build the zip.
pub fn pack(campaign: &Campaign, missions: &[(String, Scenario)]) -> Result<Vec<u8>, String> {
    if let Some(first) = problems(campaign, missions).first() {
        return Err(first.clone());
    }
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let manifest = serde_json::to_string_pretty(campaign)
            .map_err(|e| format!("the campaign does not serialise: {e}"))?;
        write_entry(&mut zip, "campaign.json", manifest.as_bytes(), options)?;
        for listed in &campaign.missions {
            let (_, scenario) = missions
                .iter()
                .find(|(id, _)| id == &listed.id)
                .ok_or_else(|| format!("no scenario for {}", listed.id))?;
            let script = mission_template(scenario)
                .map_err(|e| format!("{}: {e}", listed.name))?;
            write_entry(
                &mut zip,
                &format!("missions/{}.script", listed.id),
                script.as_bytes(),
                options,
            )?;
            let source = crate::scenario::to_json(scenario)?;
            write_entry(
                &mut zip,
                &format!("missions/{}.splaunch", listed.id),
                source.as_bytes(),
                options,
            )?;
        }
        zip.finish().map_err(|e| format!("could not finish the campaign: {e}"))?;
    }
    Ok(buffer.into_inner())
}

fn write_entry<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    name: &str,
    body: &[u8],
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    use std::io::Write;
    zip.start_file(name, options)
        .map_err(|e| format!("could not add {name}: {e}"))?;
    zip.write_all(body)
        .map_err(|e| format!("could not write {name}: {e}"))
}

/// A campaign around a single scenario.
///
/// The path from "I made a mission" to "somebody else can play it" without an
/// authoring screen in between. A campaign of one is a real campaign, and a
/// second mission is a second entry in the same list.
pub fn single(scenario: &Scenario, author: &str) -> (Campaign, Vec<(String, Scenario)>) {
    let id = slug(&scenario.name);
    let campaign = Campaign {
        format_version: current_format(),
        id: id.clone(),
        name: scenario.name.clone(),
        author: author.trim().to_string(),
        version: "1.0.0".into(),
        description: scenario.briefing.clone().unwrap_or_default(),
        // The scenario carries the archive name the editor read off this
        // machine, which is exactly the version it was compiled against.
        built_against: Some(scenario.game.clone()).filter(|g| !g.trim().is_empty()),
        missions: vec![CampaignMission {
            id: id.clone(),
            name: scenario.name.clone(),
            map: scenario.map.clone(),
            requires: vec![],
            summary: None,
        }],
    };
    (campaign, vec![(id, scenario.clone())])
}

/// Write a campaign, asking where.
///
/// Returns the path written, or `None` if the author closed the dialog, which
/// is not an error and should not be reported as one.
#[tauri::command]
pub fn spsc_export_campaign(
    app: tauri::AppHandle,
    scenario: Scenario,
    author: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (campaign, missions) = single(&scenario, &author);
    // Packed before the dialog: an author who has just chosen a filename should
    // not then be told the mission does not compile.
    let bytes = pack(&campaign, &missions)?;
    let suggested = format!("{}.{CAMPAIGN_EXT}", campaign.id);
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export campaign")
        .set_file_name(&suggested)
        .add_filter("Shiro campaign", &[CAMPAIGN_EXT])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|e| format!("that is not a path this can write to: {e}"))?;
    crate::savefile::write(&path, &bytes)?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::{from_json, Scenario};

    fn example() -> Scenario {
        from_json(include_str!("../../examples/first-contact.splaunch")).unwrap()
    }

    fn one_mission() -> (Campaign, Vec<(String, Scenario)>) {
        let campaign = Campaign {
            format_version: 1,
            id: "first-contact".into(),
            name: "First Contact".into(),
            author: "Qrow".into(),
            version: "1.0.0".into(),
            description: "One mission.".into(),
            built_against: Some("Zero-K v1.14.8.0".into()),
            missions: vec![CampaignMission {
                id: "01-first-contact".into(),
                name: "First Contact".into(),
                map: "Comet Catcher Redux".into(),
                requires: vec![],
                summary: None,
            }],
        };
        (campaign, vec![("01-first-contact".into(), example())])
    }

    #[test]
    fn every_payload_the_compiler_writes_is_one_this_checks() {
        /* `is_encoded` names the keys it knows, so a payload added to the
           compiler and not to it would be waved through in silence - which is
           the same shape as the bug the check itself exists to catch, one level
           up. The two answers are compared rather than one trusted: everything
           `customkey` encodes is a Lua table, so a payload announces itself by
           decoding to one, without reference to its name. */
        let mut sc = example();
        sc.units[0].neutral = true; // Gaia's units ride on the modoptions table

        let script = mission_template(&sc).unwrap();
        let mut checked = 0;
        for (key, value) in payloads(&script) {
            let decoded = crate::customkey::decode_as_the_game_does(&value);
            let is_a_table = value.len() >= 8 && decoded.first() == Some(&b'{');
            assert_eq!(
                is_a_table,
                is_encoded(&key),
                "{key} decodes to a Lua table but `is_encoded` does not name it, or the reverse"
            );
            checked += usize::from(is_a_table);
        }
        assert!(checked >= 5, "the example should exercise more than {checked} payloads");
    }

    #[test]
    fn a_template_carries_no_machine_of_its_own() {
        let script = mission_template(&example()).unwrap();
        assert!(script.contains(HOLE_MAP), "{script}");
        assert!(script.contains(HOLE_GAME));
        assert!(script.contains(HOLE_PLAYER));
        // The map the example was written against must not survive into it,
        // or a loader would launch somebody else's map.
        assert!(!script.contains("Comet Catcher"), "{script}");
    }

    #[test]
    fn filling_a_template_produces_the_script_the_editor_would_have_written() {
        /* The property that makes shipping compiled missions safe: a template
           filled in for a machine is byte-for-byte what this crate would have
           compiled on that machine. If that ever stops holding, campaigns and
           the Test button have diverged. */
        let mut here = example();
        here.map = "Icy Crater v4".into();
        here.game = "Zero-K v1.14.8.0".into();
        let direct = write_script(&here, "Qrow").unwrap();

        let filled = fill(
            &mission_template(&example()).unwrap(),
            "Icy Crater v4",
            "Zero-K v1.14.8.0",
            "Qrow",
        );
        assert_eq!(filled, direct);
    }

    #[test]
    fn every_payload_survives_the_games_decoder() {
        // `mission_template` refuses rather than shipping one that does not.
        let script = mission_template(&example()).expect("the example does not survive");
        assert!(script.contains("extrastartunits_1"), "{script}");
    }

    #[test]
    fn a_campaign_packs_to_a_zip_holding_both_halves_of_each_mission() {
        let (campaign, missions) = one_mission();
        let bytes = pack(&campaign, &missions).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"campaign.json".to_string()), "{names:?}");
        assert!(names.contains(&"missions/01-first-contact.script".to_string()), "{names:?}");
        assert!(names.contains(&"missions/01-first-contact.splaunch".to_string()), "{names:?}");
    }

    #[test]
    fn the_manifest_reads_back_as_the_campaign_that_was_packed() {
        use std::io::Read;
        let (campaign, missions) = one_mission();
        let bytes = pack(&campaign, &missions).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut text = String::new();
        zip.by_name("campaign.json").unwrap().read_to_string(&mut text).unwrap();
        let read: Campaign = serde_json::from_str(&text).unwrap();
        assert_eq!(read, campaign);
    }

    #[test]
    fn a_mission_that_needs_one_outside_the_campaign_is_refused() {
        let (mut campaign, missions) = one_mission();
        campaign.missions[0].requires = vec!["02-nowhere".into()];
        let said = problems(&campaign, &missions);
        assert!(said.iter().any(|p| p.contains("02-nowhere")), "{said:?}");
    }

    #[test]
    fn a_campaign_with_no_first_mission_is_refused() {
        /* Two missions each waiting on the other. Every mission is locked, the
           campaign opens to an empty list, and nothing says why. */
        let (mut campaign, mut missions) = one_mission();
        let second = CampaignMission {
            id: "02-the-outpost".into(),
            name: "The Outpost".into(),
            map: "Icy Crater v4".into(),
            requires: vec!["01-first-contact".into()],
            summary: None,
        };
        campaign.missions[0].requires = vec!["02-the-outpost".into()];
        campaign.missions.push(second);
        missions.push(("02-the-outpost".into(), example()));
        let said = problems(&campaign, &missions);
        assert!(said.iter().any(|p| p.contains("played first")), "{said:?}");
    }

    #[test]
    fn a_listed_mission_with_no_scenario_is_refused() {
        let (campaign, _) = one_mission();
        let said = problems(&campaign, &[]);
        assert!(said.iter().any(|p| p.contains("no scenario")), "{said:?}");
    }

    #[test]
    fn one_scenario_makes_a_campaign_that_packs() {
        let (campaign, missions) = single(&example(), "Qrow");
        assert_eq!(campaign.id, "first-contact");
        assert_eq!(campaign.missions.len(), 1);
        assert_eq!(campaign.missions[0].map, "Comet Catcher Redux");
        assert!(problems(&campaign, &missions).is_empty());
        assert!(!pack(&campaign, &missions).unwrap().is_empty());
    }

    /// Write a real campaign out, for testing a loader against.
    ///
    /// Ignored because it writes a file somebody has to ask for:
    ///
    /// ```text
    /// SPLAUNCH_TEST_CAMPAIGN=out.shirocamp cargo test --lib -- --ignored
    /// ```
    #[test]
    #[ignore = "writes a file named by SPLAUNCH_TEST_CAMPAIGN"]
    fn the_example_packs_to_a_file() {
        let Ok(path) = std::env::var("SPLAUNCH_TEST_CAMPAIGN") else {
            panic!("set SPLAUNCH_TEST_CAMPAIGN to the file to write");
        };
        let (campaign, missions) = single(&example(), "Qrow");
        let bytes = pack(&campaign, &missions).expect("the example does not pack");
        std::fs::write(&path, &bytes).expect("could not write it");
        println!("wrote {path} ({} bytes)", bytes.len());
    }

    #[test]
    fn a_slug_is_safe_on_every_platform() {
        assert_eq!(slug("First Contact"), "first-contact");
        assert_eq!(slug("Mission 2: The Outpost"), "mission-2-the-outpost");
        assert_eq!(slug("  ../etc/passwd  "), "etc-passwd");
        assert_eq!(slug("???"), "mission");
    }
}
