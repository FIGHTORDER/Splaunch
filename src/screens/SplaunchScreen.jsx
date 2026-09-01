import React from "react";
import {
  Button, Input, Tabs, Dialog, Icon, IconButton, EmptyState, Badge,
} from "../ds/shiro.js";
import { label, hint, mono } from "./parts.jsx";
import Minimap, { MinimapCaption, mapAspect, containBox } from "./Minimap.jsx";
import Objectives from "./Objectives.jsx";
import Teams, { colourOf, rgbOf, NEUTRAL_RGB } from "./Teams.jsx";
import Selection from "./Selection.jsx";
import { useUnitIcons, useUnitMarks, UnitMark } from "./icons.jsx";
import MetalOverlay, { useMetalMap } from "./Metal.jsx";
import {
  scenarioProblems, scenarioWarnings, scenarioScript, saveScenario, openScenario,
  exampleScenario, importPreset, exportPreset,
  mapIsInstalled,
  FORMAT_VERSION, DEFAULT_MAP_ELMOS, DEFAULT_DIFFICULTY,
} from "../net/splaunch.ts";

/* Splaunch. Place units on a map, set objectives, press Test.
 *
 * Test is not a preview: a scenario compiles to a Spring start script and
 * launches the real game into it.
 * and src-tauri/src/scenario.rs for the writer.
 *
 * The kit this is drawn from puts water and slope over the map, which is the
 * right answer and the thing that stops somebody putting a tank in the sea. We
 * have no heightmap, so drawing them would mean drawing them somewhere
 * invented. The screen says the terrain is unchecked instead. */

const sizeField = {
  width: 78, font: "var(--w-regular) var(--size-tiny)/1.4 var(--font-mono)",
  background: "var(--surface-base)", color: "var(--text-body)",
  border: "1px solid var(--w-12)", padding: "var(--sp-2) var(--sp-3)",
};

/** "20 x 12", the way Spring talks about map sizes, or nothing when unknown. */
function sizeNote(m) {
  if (!m.widthElmos || !m.heightElmos) return null;
  return `${m.widthElmos / 512} x ${m.heightElmos / 512}`;
}

/** A wreck for any unit: Zero-K names a unit's corpse after it. */
function featureNames(roster) {
  return roster
    .filter(u => u.group !== "Test and debug")
    .map(u => ({ name: `${u.name}_dead`, title: `${u.title} wreck`, group: u.group }));
}

function Palette({ mode, setMode, query, setQuery, brush, setBrush, roster, source, icons, onShown }) {
  const items = React.useMemo(
    () => (mode === "unit" ? roster : mode === "feature" ? featureNames(roster) : []),
    [mode, roster],
  );

  const groups = React.useMemo(() => {
    const q = query.trim().toLowerCase();
    const byGroup = new Map();
    for (const item of items) {
      const matches = !q
        || item.name.toLowerCase().includes(q)
        || item.title.toLowerCase().includes(q)
        || item.group.toLowerCase().includes(q);
      if (!matches) continue;
      if (!byGroup.has(item.group)) byGroup.set(item.group, []);
      byGroup.get(item.group).push(item);
    }
    return [...byGroup.entries()];
  }, [items, query]);

  /* A screenful and a bit, capped across the whole list rather than per group.
     Per group is not a cap at all: twenty-four groups of forty is the entire
     roster, which is the megabytes this exists to avoid. Searching narrows the
     list, so the way to reach a unit far down it is to type its name - which is
     how anybody finds one unit among 275 anyway. */
  const PALETTE_ICON_BUDGET = 60;
  const shown = React.useMemo(
    () => groups.flatMap(([, list]) => list)
      .slice(0, PALETTE_ICON_BUDGET)
      .map(i => i.name.replace(/_dead$/, "")),
    [groups],
  );
  React.useEffect(() => { onShown?.(shown); }, [shown, onShown]);

  return (
    <div style={{ width: 236, flex: "0 0 auto", display: "flex", flexDirection: "column",
      minHeight: 0, borderRight: "1px solid var(--w-12)", background: "var(--surface-sunken)" }}>
      <div style={{ padding: "var(--sp-5)", borderBottom: "1px solid var(--w-06)",
        display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
        <div style={{ display: "flex", gap: "var(--sp-3)" }}>
          <Button size="sm" block variant={mode === "unit" ? "primary" : "secondary"}
            onClick={() => setMode("unit")}>Units</Button>
          <Button size="sm" block variant={mode === "feature" ? "primary" : "secondary"}
            onClick={() => setMode("feature")}>Wrecks</Button>
          <Button size="sm" block variant={mode === "marker" ? "primary" : "secondary"}
            onClick={() => setMode("marker")}>Marks</Button>
        </div>
        <Input label="Palette" icon="search" placeholder="Glaive, cloakraid, Defences"
          value={query} onChange={e => setQuery(e.target.value)} />
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "var(--sp-5)" }}>
        {groups.map(([group, list]) => (
          <div key={group} style={{ marginBottom: "var(--sp-6)" }}>
            <span style={label}>{group}</span>
            <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-2)",
              marginTop: "var(--sp-4)" }}>
              {list.map(item => {
                const on = brush === item.name;
                return (
                  <button key={item.name} type="button" onClick={() => setBrush(item.name)}
                    title={`${item.title} (${item.name})`}
                    style={{ padding: "var(--sp-3)", cursor: "pointer", textAlign: "left",
                      background: on ? "var(--surface-inverse)" : "transparent",
                      color: on ? "var(--text-inverse)" : "var(--text-body)",
                      border: "1px solid " + (on ? "var(--surface-inverse)" : "var(--w-12)"),
                      overflow: "hidden", display: "flex", alignItems: "center",
                      gap: "var(--sp-3)" }}>
                    {/* The wreck palette names `<unit>_dead`; the picture is
                        the unit's, because a corpse has none of its own. */}
                    <span style={{ width: 26, height: 26, flex: "0 0 auto",
                      display: "flex", alignItems: "center", justifyContent: "center",
                      background: "rgba(0,0,0,.06)" }}>
                      {icons[item.name.replace(/_dead$/, "")] && (
                        <img src={icons[item.name.replace(/_dead$/, "")]} alt=""
                          draggable={false}
                          style={{ width: "100%", height: "100%", objectFit: "contain",
                            opacity: item.name.endsWith("_dead") ? 0.55 : 1 }} />
                      )}
                    </span>
                    <span style={{ minWidth: 0, flex: 1 }}>
                      <span style={{ font: "var(--w-medium) var(--size-tiny)/1.3 var(--font-core)",
                        display: "block", overflow: "hidden", textOverflow: "ellipsis",
                        whiteSpace: "nowrap" }}>{item.title}</span>
                      <span style={{ font: "var(--w-regular) var(--size-micro)/1.3 var(--font-mono)",
                        opacity: 0.7, display: "block", overflow: "hidden",
                        textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{item.name}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        ))}
        {mode === "marker" ? (
          <EmptyState icon="map" title="Marks."
            body="Click the map to label a place. The player sees them from the start." />
        ) : groups.length === 0 && (
          <EmptyState icon="search" title="Nothing matches that."
            body="Search by what a unit is called, or by what the engine calls it." />
        )}
      </div>

      {source && (
        <div style={{ padding: "var(--sp-4) var(--sp-5)", borderTop: "1px solid var(--w-06)" }}>
          <span style={hint}>Roster from {source}.</span>
        </div>
      )}
    </div>
  );
}

export default function SplaunchScreen({
  maps = [], roster: rosterIn = { source: "", units: [] }, install,
  player = "Player", onTest, onBack, testError, running,
}) {
  const roster = rosterIn.units ?? [];
  /* Zero-K spawns a team's commander itself, so a placed one is the team's
     start position rather than a unit. Its own grouping is the authority on
     what counts - see `game::is_commander`. */
  const commanderNames = React.useMemo(
    () => new Set(roster.filter(u => u.group === "Commanders").map(u => u.name)),
    [roster],
  );

  const [name, setName] = React.useState("Untitled scenario");
  const [map, setMap] = React.useState("");
  const [mapElmos, setMapElmos] = React.useState(DEFAULT_MAP_ELMOS);
  const [mapDepth, setMapDepth] = React.useState(DEFAULT_MAP_ELMOS);
  const [mapQuery, setMapQuery] = React.useState("");
  const [units, setUnits] = React.useState([]);
  const [features, setFeatures] = React.useState([]);
  const [goals, setGoals] = React.useState([]);
  const [notes, setNotes] = React.useState([]);
  const [briefing, setBriefing] = React.useState("");
  const [defeat, setDefeat] = React.useState([]);
  const [markers, setMarkers] = React.useState([]);
  const [modOptions, setModOptions] = React.useState({});
  const [presetNote, setPresetNote] = React.useState(null);
  const [difficulty, setDifficulty] = React.useState(DEFAULT_DIFFICULTY);
  const [routing, setRouting] = React.useState(false);
  const [teams, setTeams] = React.useState([
    { id: 0, ally: 0, ai: null, colour: colourOf(0).rgb },
    { id: 1, ally: 1, ai: "NullAI", colour: colourOf(1).rgb },
  ]);

  const [sel, setSel] = React.useState(null);
  const [mode, setMode] = React.useState("unit");
  const [brush, setBrush] = React.useState("");
  const [paletteQuery, setPaletteQuery] = React.useState("");
  const [team, setTeam] = React.useState(0);
  const [tab, setTab] = React.useState("selection");
  const [problems, setProblems] = React.useState([]);
  const [warnings, setWarnings] = React.useState([]);
  const [issuesOpen, setIssuesOpen] = React.useState(false);
  const [blockedOpen, setBlockedOpen] = React.useState(false);
  const [saved, setSaved] = React.useState(undefined);
  const [script, setScript] = React.useState(undefined);
  const [paletteShown, setPaletteShown] = React.useState([]);
  const [showMetal, setShowMetal] = React.useState(true);
  const boardRef = React.useRef(null);

  /* Build pictures for the palette, which is where a unit is recognised, and
     zoom-out silhouettes for the map, which is where one has to be read at
     twenty pixels. Different art for different sizes rather than one stretched
     to do both. */
  const icons = useUnitIcons(paletteShown);
  const metal = useMetalMap(map);
  const placedNames = React.useMemo(() => units.map(u => u.unit), [units]);
  const marks = useUnitMarks(placedNames);
  const stageRef = React.useRef(null);
  const [stage, setStage] = React.useState({ w: 0, h: 0 });

  /* The board is sized in pixels rather than by CSS. `aspect-ratio` alongside a
     width and a max-height is satisfied by breaking the ratio, and a board
     whose ratio is wrong is the bug this is fixing. */
  React.useEffect(() => {
    const el = stageRef.current;
    if (!el) return undefined;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setStage({ w: r.width, h: r.height });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return undefined;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [map]);

  /* The biggest box of the map's shape that fits the stage. Both axes are
     honoured, so a 12x4 map is a strip and a 6x16 one is a column. */
  const board = React.useMemo(() => {
    const aspect = mapElmos / Math.max(1, mapDepth);
    const pad = 2; // the board's own border, which sits inside the stage
    const w = Math.max(0, Math.min(stage.w - pad, (stage.h - pad) * aspect));
    return { width: Math.floor(w), height: Math.floor(w / aspect) };
  }, [stage, mapElmos, mapDepth]);

  /* Route strokes are in viewBox units, and the viewBox is no longer square.
     Keying them to the longer side keeps them the same weight on every map. */
  const mapSpan = Math.max(mapElmos, mapDepth);

  /* Where Zero-K will put each team's commander. Mirrors `team_start` in
     scenario.rs, which is the authority - this only draws it. Shown because the
     rule is otherwise invisible: the game spawns a commander whether or not one
     was placed, and an author who placed none had no way to see that it was
     going to appear in the middle of the map. */
  const starts = React.useMemo(() => teams.map(t => {
    const mine = units.filter(u => !u.neutral && u.team === t.id);
    const commander = mine.find(u => commanderNames.has(u.unit));
    if (commander) return { team: t.id, x: commander.x, z: commander.z, placed: true };
    if (!mine.length) return { team: t.id, x: mapElmos / 2, z: mapDepth / 2, placed: false };
    return {
      team: t.id,
      x: mine.reduce((a, u) => a + u.x, 0) / mine.length,
      z: mine.reduce((a, u) => a + u.z, 0) / mine.length,
      placed: false,
    };
  }), [teams, units, commanderNames, mapElmos, mapDepth]);

  /* A brush as soon as there is a roster to pick one from - a commander, not
     whatever happens to sort first. Nearly every scenario starts by placing
     one, and the alternative was the Airplane Plant's construction aircraft. */
  React.useEffect(() => {
    if (brush || !roster.length) return;
    const commander = roster.find(u => u.group === "Commanders" && u.name === "armcom1")
      ?? roster.find(u => u.group === "Commanders")
      ?? roster[0];
    setBrush(commander.name);
  }, [roster, brush]);
  React.useEffect(() => {
    if (mode === "feature" && brush && !brush.endsWith("_dead")) setBrush(`${brush}_dead`);
    if (mode === "unit" && brush.endsWith("_dead")) setBrush(brush.replace(/_dead$/, ""));
  }, [mode]); // eslint-disable-line react-hooks/exhaustive-deps

  const scenario = React.useMemo(() => ({
    formatVersion: FORMAT_VERSION,
    name,
    map,
    game: install?.game ?? "",
    teams,
    units,
    objectives: notes,
    goals,
    features,
    briefing: briefing.trim() ? briefing : null,
    defeat,
    mapElmos,
    /* Only when it says something. A square map writes no depth at all, so a
       scenario that does not need the field still opens in a Splaunch that
       predates it. */
    mapElmosZ: mapDepth === mapElmos ? null : mapDepth,
    modOptions,
    markers,
    difficulty,
  }), [name, map, install, teams, units, notes, goals, features, briefing, defeat,
    mapElmos, mapDepth, modOptions, markers, difficulty]);

  /* The Rust side is the authority on what is wrong. The editor used to keep
     its own list, which drifted: a scenario could pass every check an author
     could see and still be refused by the writer. */
  React.useEffect(() => {
    let live = true;
    scenarioProblems(scenario).then(p => { if (live) setProblems(p); }, () => {});
    scenarioWarnings(scenario).then(w => { if (live) setWarnings(w); }, () => {});
    return () => { live = false; };
  }, [scenario]);

  const load = sc => {
    setName(sc.name ?? "Untitled scenario");
    setMap(sc.map ?? "");
    setMapElmos(sc.mapElmos || DEFAULT_MAP_ELMOS);
    setMapDepth(sc.mapElmosZ || sc.mapElmos || DEFAULT_MAP_ELMOS);
    setUnits((sc.units ?? []).map((u, i) => ({ ...u, key: `u${i}` })));
    setFeatures((sc.features ?? []).map((f, i) => ({ ...f, key: `f${i}` })));
    setGoals(sc.goals ?? []);
    setNotes(sc.objectives ?? []);
    setBriefing(sc.briefing ?? "");
    setDefeat(sc.defeat ?? []);
    setMarkers((sc.markers ?? []).map((m, i) => ({ ...m, key: `m${i}` })));
    setModOptions(sc.modOptions ?? {});
    setDifficulty(sc.difficulty || DEFAULT_DIFFICULTY);
    setTeams(sc.teams?.length ? sc.teams : teams);
    setSel(null);
    setRouting(false);
  };

  const place = e => {
    const box = boardRef.current?.getBoundingClientRect();
    if (!box) return;
    const x = Math.round(Math.min(1, Math.max(0, (e.clientX - box.left) / box.width)) * mapElmos);
    const z = Math.round(Math.min(1, Math.max(0, (e.clientY - box.top) / box.height)) * mapDepth);

    /* Drawing a route takes over the map: while it is on, a click adds a point
       to the selected unit's patrol rather than placing anything new. */
    if (routing && sel?.kind === "unit") {
      setUnits(v => v.map(u =>
        u.key === sel.key ? { ...u, patrol: [...(u.patrol ?? []), [x, z]] } : u));
      return;
    }

    const key = `${mode}${Date.now()}`;
    if (mode === "marker") {
      setMarkers(v => [...v, { x, z, text: `Mark ${v.length + 1}`, key }]);
      setSel({ kind: "marker", key });
    } else if (mode === "feature") {
      if (!brush) return;
      setFeatures(v => [...v, { name: brush, x, z, facing: null, key }]);
      setSel({ kind: "feature", key });
    } else {
      if (!brush) return;
      setUnits(v => [...v, { unit: brush, team, x, z, key }]);
      setSel({ kind: "unit", key });
    }
    setTab("selection");
  };

  const pool = { unit: units, feature: features, marker: markers };
  const selected = sel ? pool[sel.kind]?.find(x => x.key === sel.key) ?? null : null;

  const setterFor = kind =>
    kind === "feature" ? setFeatures : kind === "marker" ? setMarkers : setUnits;

  const patchSelected = next => {
    if (!sel) return;
    setterFor(sel.kind)(v => v.map(x => (x.key === sel.key ? { ...x, ...next } : x)));
  };

  const deleteSelected = () => {
    if (!sel) return;
    setterFor(sel.kind)(v => v.filter(x => x.key !== sel.key));
    setSel(null);
    setRouting(false);
  };

  const test = () => {
    if (running) { setBlockedOpen(true); return; }
    if (problems.length) { setIssuesOpen(true); return; }
    onTest?.(scenario);
  };

  /* Installed maps first. Zero-K downloads maps on demand, so the catalogue
     lists 343 and an install has a handful - picking one you do not have gets
     you an error from the engine about an archive, which is a poor way to find
     out you needed to play the map once first. */
  const onDisk = install?.maps ?? [];
  const shownMaps = React.useMemo(() => {
    const q = mapQuery.trim().toLowerCase();
    return maps
      .filter(m => !q || m.name.toLowerCase().includes(q))
      .map(m => ({ ...m, installed: mapIsInstalled(onDisk, m.name) }))
      .sort((a, b) => (b.installed ? 1 : 0) - (a.installed ? 1 : 0));
  }, [maps, mapQuery, onDisk]);

  const chosenMissing = map && onDisk.length > 0 && !mapIsInstalled(onDisk, map);

  const chooseMap = m => {
    setMap(m.name);
    /* The catalogue knows how big the map is, on both axes. Everything used to
       be placed against a hardcoded 8x8, so a click in the middle of a 16x16
       map landed in its top-left quarter; then against one figure for both
       axes, which is wrong on the 145 catalogue maps of 343 that are not
       square. Where the catalogue does not say, the default stands. */
    setMapElmos(m.widthElmos ?? m.heightElmos ?? DEFAULT_MAP_ELMOS);
    setMapDepth(m.heightElmos ?? m.widthElmos ?? DEFAULT_MAP_ELMOS);
    if (name === "Untitled scenario") setName(`${m.name} scenario`);
  };

  const openFile = () => openScenario().then(sc => { if (sc) load(sc); }, () => {});
  const openExample = () => exampleScenario().then(load, () => {});

  /* Coilbox's bottom layer: a preset is the battle - map, game, who is playing,
     modoptions - and a scenario is what happens inside it. So a preset is laid
     over what is already here rather than replacing it, and whatever could not
     come with it is said out loud. See vendor/coilbox/INTEROP.md. */
  const openPreset = (text) => importPreset(scenario, text).then(applied => {
    if (!applied) return;
    load(applied.scenario);
    setPresetNote({ name: applied.name, ignored: applied.ignored });
  }, e => setPresetNote({ name: null, ignored: [String(e)] }));

  const savePreset = () => exportPreset(scenario).then(
    p => { if (p) setSaved(`Preset written to ${p}`); },
    e => setPresetNote({ name: null, ignored: [String(e)] }));

  if (!map) {
    return (
      <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
        <div style={{ height: 44, flex: "0 0 auto", display: "flex", alignItems: "center",
          gap: "var(--sp-5)", padding: "0 var(--sp-6)", borderBottom: "1px solid var(--w-12)" }}>
          {onBack && <Button variant="ghost" size="sm" icon="arrow-left" onClick={onBack}>Apps</Button>}
          <span style={label}>SPLAUNCH — NEW SCENARIO</span>
          <span style={{ flex: 1 }} />
          <Button variant="secondary" size="sm" onClick={() => openPreset()}>Coilbox preset</Button>
          <Button variant="secondary" size="sm" onClick={openExample}>Open the example</Button>
          <Button variant="secondary" size="sm" icon="folder" onClick={openFile}>Open</Button>
        </div>
        <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "var(--sp-7)" }}>
          <div style={{ maxWidth: 420, marginBottom: "var(--sp-6)" }}>
            <Input label="Choose a map" icon="search" placeholder="Search maps"
              value={mapQuery} onChange={e => setMapQuery(e.target.value)} />
          </div>
          {testError && (
            <div style={{ marginBottom: "var(--sp-6)", padding: "var(--sp-5)",
              border: "1px solid var(--signal-warn)", maxWidth: 640 }}>
              <span style={{ font: "var(--text-ui-sm)", color: "var(--signal-warn)" }}>{testError}</span>
            </div>
          )}
          {shownMaps.length === 0 ? (
            <EmptyState icon="map" title="No maps known yet."
              body="The catalogue is fetched from zero-k.info when Splaunch starts." />
          ) : (
            <div style={{ display: "grid",
              gridTemplateColumns: "repeat(auto-fill,minmax(180px,1fr))", gap: "var(--sp-5)" }}>
              {shownMaps.slice(0, 48).map(m => (
                <button key={m.name} type="button" onClick={() => chooseMap(m)} aria-label={m.name}
                  style={{ cursor: "pointer", background: "transparent", border: 0,
                    padding: 0, textAlign: "left", color: "inherit", font: "inherit",
                    opacity: onDisk.length && !m.installed ? 0.5 : 1, position: "relative" }}>
                  {/* A square slot so the grid stays a grid, with the map
                      letterboxed inside it at its real proportions. Cropping
                      to the slot hid that a map is a 3:1 strip until after it
                      was chosen. */}
                  <div style={{ position: "relative", aspectRatio: "1",
                    background: "var(--ink-000)", overflow: "hidden", display: "flex",
                    alignItems: "center", justifyContent: "center" }}>
                    <Minimap map={m.name}
                      style={containBox(mapAspect(m.widthElmos, m.heightElmos))} />
                    <MinimapCaption map={m.name} note={sizeNote(m)} />
                  </div>
                  {onDisk.length > 0 && !m.installed && (
                    <span style={{ position: "absolute", top: 6, left: 6,
                      background: "var(--surface-base)", color: "var(--text-faint)",
                      font: "var(--w-semibold) var(--size-micro)/1 var(--font-core)",
                      letterSpacing: "var(--track-label)", textTransform: "uppercase",
                      padding: "3px 5px", border: "1px solid var(--w-12)" }}>
                      Not installed
                    </span>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
      <div style={{ height: 44, flex: "0 0 auto", display: "flex", alignItems: "center",
        gap: "var(--sp-4)", padding: "0 var(--sp-6)", borderBottom: "1px solid var(--w-12)" }}>
        {onBack && <Button variant="ghost" size="sm" icon="arrow-left" onClick={onBack}>Apps</Button>}
        <input value={name} onChange={e => setName(e.target.value)} aria-label="Scenario name"
          style={{ font: "var(--w-semibold) var(--size-base)/1 var(--font-core)",
            color: "var(--text-hi)", background: "transparent", border: 0, outline: "none",
            width: 220, padding: "var(--sp-2) 0" }} />
        <span style={{ ...mono, color: "var(--text-faint)" }}>{map}</span>
        {chosenMissing && (
          <Badge tone="warn">Map not installed</Badge>
        )}
        <Button variant="ghost" size="sm"
          onClick={() => { setMap(""); setSel(null); }}>Change map</Button>
        <span style={{ flex: 1 }} />
        {install?.game
          ? <Badge tone="neutral" mono>{install.game}</Badge>
          : <Badge tone="danger">No Zero-K found</Badge>}
        {/* What will actually be handed to the engine. The one thing worth
            looking at when a launch does something unexpected, and it costs
            nothing to expose because the compiler already answers this. */}
        <Button variant="ghost" size="sm" icon="code"
          onClick={() => scenarioScript(scenario, player).then(
            t => setScript(t || "(nothing - fix the problems first)"),
            e => setScript(String(e?.message ?? e)))}>Script</Button>
        <Button variant="ghost" size="sm" icon="folder" onClick={openFile}>Open</Button>
        <Button variant="ghost" size="sm" icon="share" onClick={savePreset}
          title="Write the battle - map, game, sides, modoptions - as a Coilbox preset">
          Preset</Button>
        {/* The packer has been in the Rust with nothing able to reach it. A
            campaign is how a mission gets to somebody else's machine: the
            script travels compiled, with the map, the game and the player
            written as markers for the loader to substitute. */}
        <Button variant="ghost" size="sm" icon="package"
          onClick={() => exportCampaign(scenario, player).then(
            p => setSaved(p ? `Campaign written to ${p}` : undefined),
            e => setSaved(`Could not write the campaign: ${String(e?.message ?? e)}`))}>Campaign</Button>
        <Button variant="ghost" size="sm" icon="save"
          onClick={() => saveScenario(scenario).then(
            p => setSaved(p ? `Saved to ${p}` : undefined), () => {})}>Save</Button>
        {problems.length > 0 && (
          <button type="button" onClick={() => setIssuesOpen(true)}
            style={{ background: "none", border: "1px solid var(--signal-danger)", cursor: "pointer",
              height: 20, padding: "0 var(--sp-3)", color: "var(--signal-danger)",
              font: "var(--w-semibold) var(--size-micro)/1 var(--font-core)",
              letterSpacing: "var(--track-label)", textTransform: "uppercase" }}>
            {problems.length} problem{problems.length > 1 ? "s" : ""}
          </button>
        )}
        {problems.length === 0 && warnings.length > 0 && (
          <button type="button" onClick={() => setIssuesOpen(true)}
            style={{ background: "none", border: "1px solid var(--signal-warn)", cursor: "pointer",
              height: 20, padding: "0 var(--sp-3)", color: "var(--signal-warn)",
              font: "var(--w-semibold) var(--size-micro)/1 var(--font-core)",
              letterSpacing: "var(--track-label)", textTransform: "uppercase" }}>
            {warnings.length} to know
          </button>
        )}
        <Button variant="primary" size="sm" icon="play" onClick={test}
          loading={running}>{running ? "Running" : "Test"}</Button>
      </div>

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        <Palette mode={mode} setMode={setMode} query={paletteQuery} setQuery={setPaletteQuery}
          brush={brush} setBrush={setBrush} roster={roster} source={rosterIn.source}
          icons={icons} onShown={setPaletteShown} />

        <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", minHeight: 0 }}>
          <div ref={stageRef}
            style={{ position: "relative", flex: 1, minHeight: 0, display: "flex",
              alignItems: "center", justifyContent: "center", padding: "var(--sp-5)" }}>
            <div ref={boardRef} onClick={place}
              style={{ position: "relative", width: board.width, height: board.height,
                cursor: "crosshair", border: "1px solid var(--w-20)" }}>
              <Minimap map={map} saturate={0.7}
                style={{ position: "absolute", inset: 0 }} />

              {showMetal && <MetalOverlay metal={metal} />}

              {/* Patrol routes, under everything so the pieces stay clickable. */}
              <svg viewBox={`0 0 ${mapElmos} ${mapDepth}`} preserveAspectRatio="none"
                style={{ position: "absolute", inset: 0, width: "100%", height: "100%",
                  pointerEvents: "none" }}>
                {units.filter(u => (u.patrol?.length ?? 0) > 1).map(u => (
                  <polyline key={u.key}
                    points={[[u.x, u.z], ...u.patrol].map(p => p.join(",")).join(" ")}
                    fill="none" stroke={u.neutral ? "#8b8b8b" : colourOf(u.team).css}
                    strokeWidth={mapSpan / 400} strokeDasharray={`${mapSpan / 120}`}
                    opacity={0.85} />
                ))}
              </svg>

              {/* Where each team's commander arrives. Hollow when nobody placed
                  one, because then the position is Splaunch's guess. */}
              {starts.map(s => (
                <div key={`start${s.team}`} title={s.placed
                  ? `Team ${s.team} starts here - its commander`
                  : `Team ${s.team} starts here - no commander placed, so this is a guess`}
                  style={{ position: "absolute", pointerEvents: "none",
                    left: `${(s.x / mapElmos) * 100}%`, top: `${(s.z / mapDepth) * 100}%`,
                    transform: "translate(-50%, -50%)", width: 26, height: 26,
                    borderRadius: "50%", boxSizing: "border-box",
                    border: `2px ${s.placed ? "solid" : "dashed"} ${colourOf(s.team).css}`,
                    opacity: s.placed ? 0.75 : 0.95 }} />
              ))}

              {markers.map(m => (
                <button key={m.key} type="button" title={m.text}
                  onClick={e => { e.stopPropagation(); setSel({ kind: "marker", key: m.key }); setTab("selection"); }}
                  style={{ position: "absolute",
                    left: `${(m.x / mapElmos) * 100}%`, top: `${(m.z / mapDepth) * 100}%`,
                    transform: "translate(-50%, -100%)", padding: "1px 4px", cursor: "pointer",
                    background: "rgba(0,0,0,.72)", color: "#fff", border: 0, whiteSpace: "nowrap",
                    font: "var(--w-medium) var(--size-micro)/1.4 var(--font-core)",
                    outline: sel?.key === m.key ? "2px solid #fff" : "none" }}>
                  {m.text}
                </button>
              ))}

              {features.map(f => (
                <button key={f.key} type="button" title={`${f.name} (${f.x}, ${f.z})`}
                  onClick={e => { e.stopPropagation(); setSel({ kind: "feature", key: f.key }); setTab("selection"); }}
                  style={{ position: "absolute",
                    left: `${(f.x / mapElmos) * 100}%`, top: `${(f.z / mapDepth) * 100}%`,
                    transform: "translate(-50%, -50%) rotate(45deg)",
                    width: 9, height: 9, padding: 0, cursor: "pointer",
                    border: "1px solid rgba(0,0,0,.7)", background: "#b0a58c",
                    outline: sel?.key === f.key ? "2px solid #fff" : "none" }} />
              ))}

              {units.map(u => (
                <button key={u.key} type="button"
                  title={`${roster.find(r => r.name === u.unit)?.title ?? u.unit} (${u.x}, ${u.z})`}
                  onClick={e => { e.stopPropagation(); setSel({ kind: "unit", key: u.key }); setTab("selection"); }}
                  style={{ position: "absolute",
                    left: `${(u.x / mapElmos) * 100}%`, top: `${(u.z / mapDepth) * 100}%`,
                    transform: "translate(-50%, -50%)",
                    padding: 0, cursor: "pointer", border: 0, background: "none",
                    lineHeight: 0 }}>
                  <UnitMark
                    mark={marks[u.unit]}
                    colour={u.neutral ? NEUTRAL_RGB : rgbOf(u.team)}
                    selected={sel?.key === u.key}
                    neutral={u.neutral}
                  />
                </button>
              ))}
            </div>
          </div>

          <div style={{ flex: "0 0 auto", display: "flex", alignItems: "center",
            gap: "var(--sp-5)", padding: "var(--sp-4) var(--sp-5)",
            borderTop: "1px solid var(--w-12)", flexWrap: "wrap" }}>
            {mode === "unit" && (
              <div style={{ display: "flex", gap: "var(--sp-2)" }}>
                {teams.map(t => (
                  <button key={t.id} type="button" onClick={() => setTeam(t.id)}
                    title={t.ai ? `${colourOf(t.id).name} (${t.ai})` : `${colourOf(t.id).name} (you)`}
                    style={{ width: 20, height: 20, cursor: "pointer", padding: 0,
                      background: colourOf(t.id).css,
                      border: team === t.id ? "2px solid var(--text-hi)" : "1px solid var(--w-20)" }} />
                ))}
              </div>
            )}
            <span style={{ font: "var(--w-regular) var(--size-micro)/1 var(--font-core)",
              color: routing ? "var(--signal-warn)" : "var(--text-low)",
              textTransform: "uppercase", letterSpacing: "var(--track-label)" }}>
              {routing
                ? "Click to add patrol points"
                : mode === "marker"
                  ? "Click to place a mark"
                  : `Click to place ${brush || "—"}`}
            </span>
            <span style={{ flex: 1 }} />
            {metal && (
              <Button size="sm" variant={showMetal ? "primary" : "secondary"}
                onClick={() => setShowMetal(v => !v)}>Metal</Button>
            )}
            <span style={label}>
              {units.length} units · {features.length} wrecks · {markers.length} marks
            </span>
            <label style={{ display: "flex", alignItems: "center", gap: "var(--sp-3)" }}>
              <span style={label}>Map size</span>
              <input type="number" min={512} step={512} value={mapElmos} aria-label="Map width in elmos"
                onChange={e => setMapElmos(Math.max(512, Number(e.target.value) || DEFAULT_MAP_ELMOS))}
                style={sizeField} />
              <span style={{ ...label, opacity: 0.7 }}>x</span>
              <input type="number" min={512} step={512} value={mapDepth} aria-label="Map depth in elmos"
                onChange={e => setMapDepth(Math.max(512, Number(e.target.value) || DEFAULT_MAP_ELMOS))}
                style={sizeField} />
            </label>
          </div>
        </div>

        <div style={{ width: 320, flex: "0 0 auto", borderLeft: "1px solid var(--w-12)",
          background: "var(--surface-panel)", display: "flex", flexDirection: "column",
          minHeight: 0 }}>
          <Tabs value={tab} onChange={setTab} items={[
            { id: "selection", label: "Selection" },
            { id: "objectives", label: "Objectives", unread: goals.length || undefined },
            { id: "teams", label: "Teams" },
            { id: "briefing", label: "Briefing" },
          ]} />
          <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
            {tab === "selection" && (
              <Selection selected={selected} kind={sel?.kind} teams={teams} roster={roster}
                onPatch={patchSelected} onDelete={deleteSelected}
                routing={routing} onRoute={setRouting} />
            )}
            {tab === "objectives" && (
              <Objectives goals={goals} setGoals={setGoals} notes={notes} setNotes={setNotes}
                roster={roster} />
            )}
            {tab === "teams" && (
              <Teams teams={teams} setTeams={setTeams} defeat={defeat} setDefeat={setDefeat}
                ais={install?.ais ?? []} roster={roster} units={units}
                difficulty={difficulty} setDifficulty={setDifficulty} />
            )}
            {tab === "briefing" && (
              <div style={{ padding: "var(--sp-5)", display: "flex", flexDirection: "column",
                gap: "var(--sp-4)" }}>
                <span style={label}>Briefing</span>
                <span style={hint}>
                  Shown in Zero-K's briefing window before the match starts, under
                  the scenario's name. The notes on the Objectives tab appear
                  beside it.
                </span>
                <textarea value={briefing} onChange={e => setBriefing(e.target.value)}
                  rows={14} placeholder="The dam will not hold."
                  style={{ font: "var(--text-ui-sm)", color: "var(--text-body)",
                    background: "var(--surface-base)", border: "1px solid var(--w-12)",
                    padding: "var(--sp-4)", resize: "vertical", lineHeight: 1.5 }} />
                <span style={hint}>
                  Nothing speaks during a match: the modern mission system has no
                  in-game dialogue, so this window is where the words go.
                </span>
              </div>
            )}
          </div>

          <div style={{ flex: "0 0 auto", padding: "var(--sp-5)",
            borderTop: "1px solid var(--w-06)", display: "flex",
            flexDirection: "column", gap: "var(--sp-2)" }}>
            <span style={label}>Terrain</span>
            <span style={hint}>
              Positions are placed against the minimap, so nothing here knows yet
              where the water is or how steep the ground gets. Check when you test.
            </span>
          </div>
        </div>
      </div>

      <Dialog open={!!presetNote} width={460}
        title={presetNote?.name ? `Preset: ${presetNote.name}` : "That preset did not open"}
        onClose={() => setPresetNote(null)}
        footer={<Button variant="primary" onClick={() => setPresetNote(null)}>Right</Button>}>
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          {presetNote?.name && (
            <span style={{ font: "var(--text-ui-sm)", color: "var(--text-body)", lineHeight: 1.5 }}>
              The map, the sides and the modoptions came across. Everything you had
              placed is untouched - a preset is the battle, not what happens in it.
            </span>
          )}
          {(presetNote?.ignored ?? []).map(t => (
            <div key={t} style={{ display: "flex", gap: "var(--sp-4)", alignItems: "flex-start" }}>
              <Icon name="info" size={14} style={{ color: "var(--signal-warn)", marginTop: 2 }} />
              <span style={{ font: "var(--text-ui-sm)", color: "var(--text-low)",
                lineHeight: 1.5 }}>{t}</span>
            </div>
          ))}
          {presetNote?.name && (presetNote?.ignored ?? []).length === 0 && (
            <span style={{ font: "var(--text-ui-sm)", color: "var(--text-low)" }}>
              Nothing was lost on the way in.
            </span>
          )}
        </div>
      </Dialog>

      <Dialog open={issuesOpen} title="Before you test" width={460}
        onClose={() => setIssuesOpen(false)}
        footer={<Button variant="primary" onClick={() => setIssuesOpen(false)}>Back to the map</Button>}>
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          {problems.map(t => (
            <div key={t} style={{ display: "flex", gap: "var(--sp-4)", alignItems: "flex-start" }}>
              <Icon name="alert-triangle" size={14}
                style={{ color: "var(--signal-danger)", marginTop: 2 }} />
              <span style={{ font: "var(--text-ui-sm)", color: "var(--text-body)",
                lineHeight: 1.5 }}>{t}</span>
            </div>
          ))}
          {warnings.map(t => (
            <div key={t} style={{ display: "flex", gap: "var(--sp-4)", alignItems: "flex-start" }}>
              <Icon name="info" size={14}
                style={{ color: "var(--signal-warn)", marginTop: 2 }} />
              <span style={{ font: "var(--text-ui-sm)", color: "var(--text-low)",
                lineHeight: 1.5 }}>{t}</span>
            </div>
          ))}
          {problems.length === 0 && warnings.length === 0 && (
            <span style={{ font: "var(--text-ui-sm)", color: "var(--text-low)" }}>
              Nothing to report.
            </span>
          )}
          {testError && (
            <span style={{ font: "var(--text-ui-sm)", color: "var(--signal-warn)" }}>{testError}</span>
          )}
        </div>
      </Dialog>

      <Dialog open={blockedOpen} title="Cannot test" width={400}
        onClose={() => setBlockedOpen(false)}
        footer={<Button variant="primary" onClick={() => setBlockedOpen(false)}>Close</Button>}>
        <span style={{ font: "var(--text-ui-sm)", color: "var(--text-body)", lineHeight: 1.55 }}>
          Zero-K is already running. Testing starts a new game, and the engine will
          only run one at a time. Quit the running match first.
        </span>
      </Dialog>

      <Dialog open={script !== undefined} title="The start script" width={760}
        onClose={() => setScript(undefined)}
        footer={<Button variant="primary" onClick={() => setScript(undefined)}>Close</Button>}>
        <pre style={{ ...mono, background: "var(--surface-sunken)", padding: "var(--sp-5)",
          border: "1px solid var(--w-06)", maxHeight: "60vh", overflow: "auto",
          whiteSpace: "pre", margin: 0 }}>{script}</pre>
      </Dialog>

      <Dialog open={!!saved} title="Written" width={480} onClose={() => setSaved(undefined)}
        footer={<Button variant="primary" onClick={() => setSaved(undefined)}>Close</Button>}>
        <span style={{ ...mono, wordBreak: "break-all" }}>{saved}</span>
      </Dialog>
    </div>
  );
}
