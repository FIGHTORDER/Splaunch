import React from "react";
import { Button, Select, Input, IconButton, Checkbox, EmptyState } from "../ds/shiro.js";
import { label, hint, mono } from "./parts.jsx";
import { colourOf } from "./Teams.jsx";

/* What one placed thing is, and everything about it the game will honour.
 *
 * The fields below are the ones `mission_galaxy_campaign_battle.lua` reads off
 * a placed unit. They are optional in the payload and optional here: the gadget
 * branches on a field being present, so leaving one alone is different from
 * setting it to its default. A `buildProgress` of 0 is an unbuilt husk. */

const FACINGS = [
  { value: "", label: "Random" },
  { value: "0", label: "South" },
  { value: "1", label: "East" },
  { value: "2", label: "North" },
  { value: "3", label: "West" },
];

const MOVESTATES = [
  { value: "", label: "Default" },
  { value: "0", label: "Hold position" },
  { value: "1", label: "Maneuver" },
  { value: "2", label: "Roam" },
];

function num(value) {
  return value === "" || value == null ? null : Number(value);
}

const DIFFICULTIES = [
  { value: "", label: "Every difficulty" },
  { value: "1", label: "Easy" },
  { value: "2", label: "Normal" },
  { value: "3", label: "Hard" },
];

export default function Selection({
  selected, kind, teams, roster, onPatch, onDelete, routing, onRoute,
}) {
  if (!selected) {
    /* On screen more than any other state in the tool, so it says what to do
       rather than sitting blank. */
    return (
      <EmptyState icon="target" title="Nothing selected."
        body="Pick something on the left, then click the map to place it. Click a placed unit to edit it."
        style={{ padding: "var(--sp-8) var(--sp-6)" }} />
    );
  }

  const def = roster.find(u => u.name === selected.unit);
  const box = { display: "flex", flexDirection: "column", gap: "var(--sp-5)",
    padding: "var(--sp-6) var(--sp-5)" };

  if (kind === "marker") {
    return (
      <div style={box}>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-4)" }}>
          <span style={{ font: "var(--text-heading)", color: "var(--text-hi)", flex: 1 }}>Mark</span>
          <IconButton icon="x" label="Delete" size="sm" onClick={onDelete} />
        </div>
        <Input label="Text" size="sm" value={selected.text}
          onChange={e => onPatch({ text: e.target.value })} />
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr",
          gap: "var(--sp-3) var(--sp-5)", alignItems: "baseline" }}>
          <span style={label}>Position</span>
          <span style={mono}>{Math.round(selected.x)}, {Math.round(selected.z)} elmos</span>
        </div>
        <span style={hint}>
          Shown on the player's map from the moment the mission starts.
        </span>
      </div>
    );
  }

  if (kind === "feature") {
    const resurrectable = /_dead$/i.test(selected.name);
    return (
      <div style={box}>
        <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-4)" }}>
          <span style={{ font: "var(--text-heading)", color: "var(--text-hi)", flex: 1 }}>
            {selected.name}
          </span>
          <IconButton icon="x" label="Delete" size="sm" onClick={onDelete} />
        </div>
        <Select label="Facing" size="sm" value={selected.facing == null ? "" : String(selected.facing)}
          onChange={e => onPatch({ facing: num(e.target.value) })} options={FACINGS} />
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr",
          gap: "var(--sp-3) var(--sp-5)", alignItems: "baseline" }}>
          <span style={label}>Position</span>
          <span style={mono}>{Math.round(selected.x)}, {Math.round(selected.z)} elmos</span>
        </div>
        <span style={hint}>
          {resurrectable
            ? "Ends in _dead, so Zero-K wires it up as a reclaimable wreck that resurrects into the unit it came from."
            : "Scenery. A feature whose name ends in _dead resurrects into the unit it came from."}
        </span>
      </div>
    );
  }

  return (
    <div style={box}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-4)" }}>
        <span style={{ width: 10, height: 10, flex: "0 0 auto",
          background: selected.neutral ? "#8b8b8b" : colourOf(selected.team).css }} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <span style={{ font: "var(--text-heading)", color: "var(--text-hi)", display: "block" }}>
            {def?.title ?? selected.unit}
          </span>
          <span style={{ ...mono, color: "var(--text-faint)" }}>{selected.unit}</span>
        </div>
        <IconButton icon="x" label="Delete" size="sm" onClick={onDelete} />
      </div>

      {def?.description && <span style={hint}>{def.description}</span>}

      <Select label="Team" size="sm" value={String(selected.team)}
        onChange={e => onPatch({ team: Number(e.target.value) })}
        options={teams.map(t => ({
          value: String(t.id),
          label: `${colourOf(t.id).name}${t.ai ? ` (${t.ai})` : " (you)"}`,
        }))} />

      <Checkbox label="Neutral (owned by Gaia)" checked={!!selected.neutral}
        onChange={e => onPatch({ neutral: e.target.checked })} />

      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr",
        gap: "var(--sp-3) var(--sp-5)", alignItems: "baseline" }}>
        <span style={label}>Position</span>
        <span style={mono}>{Math.round(selected.x)}, {Math.round(selected.z)} elmos</span>
      </div>

      <Select label="Facing" size="sm" value={selected.facing == null ? "" : String(selected.facing)}
        onChange={e => onPatch({ facing: num(e.target.value) })} options={FACINGS} />

      <Select label="Stance" size="sm"
        value={selected.movestate == null ? "" : String(selected.movestate)}
        onChange={e => onPatch({ movestate: num(e.target.value) })} options={MOVESTATES} />

      <Input label="Built" size="sm" type="number" min={0} max={100} step={5}
        hint="Per cent. Empty means finished; 0 is an unbuilt husk."
        value={selected.buildProgress == null ? "" : String(Math.round(selected.buildProgress * 100))}
        onChange={e => {
          const raw = e.target.value;
          onPatch({ buildProgress: raw === "" ? null : Math.min(1, Math.max(0, Number(raw) / 100)) });
        }} />

      <Input label="Experience" size="sm" type="number" min={0} step={0.5}
        hint="Veterancy. Empty means none."
        value={selected.experience == null ? "" : String(selected.experience)}
        onChange={e => onPatch({ experience: num(e.target.value) })} />

      <Checkbox label="Invincible" checked={!!selected.invincible}
        onChange={e => onPatch({ invincible: e.target.checked ? true : null })} />

      <Checkbox label="Flatten the ground under it" checked={selected.terraformHeight != null}
        onChange={e => onPatch({ terraformHeight: e.target.checked ? 0 : null })} />

      {/* The closest the modern mission system comes to scripted behaviour.
          There is no trigger graph to hang orders off, so a route walked for
          ever is what a sentry or a sweep has to be built out of. */}
      <div style={{ borderTop: "1px solid var(--w-06)", paddingTop: "var(--sp-5)",
        display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
        <span style={label}>Patrol</span>
        {(selected.patrol?.length ?? 0) > 0 ? (
          <>
            <span style={mono}>
              {selected.patrol.length} point{selected.patrol.length === 1 ? "" : "s"}
              {selected.patrol.length === 1 ? " — needs two to be a route" : ""}
            </span>
            <div style={{ display: "flex", gap: "var(--sp-3)" }}>
              <Button size="sm" variant={routing ? "primary" : "secondary"}
                onClick={() => onRoute(!routing)}>
                {routing ? "Done" : "Add points"}
              </Button>
              <Button size="sm" variant="secondary"
                onClick={() => onPatch({ patrol: [] })}>Clear</Button>
            </div>
          </>
        ) : (
          <>
            <Button size="sm" variant={routing ? "primary" : "secondary"}
              onClick={() => onRoute(!routing)}>
              {routing ? "Click the map, then Done" : "Draw a route"}
            </Button>
            <Checkbox label="Patrol on the spot" checked={!!selected.selfPatrol}
              hint="Faces the middle of the map. A route replaces this."
              onChange={e => onPatch({ selfPatrol: e.target.checked })} />
          </>
        )}
      </div>

      <div style={{ borderTop: "1px solid var(--w-06)", paddingTop: "var(--sp-5)",
        display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
        <span style={label}>Only exists on</span>
        <Select size="sm" label="At least"
          value={selected.difficultyAtLeast == null ? "" : String(selected.difficultyAtLeast)}
          onChange={e => onPatch({ difficultyAtLeast: num(e.target.value) })}
          options={DIFFICULTIES} />
        <Select size="sm" label="At most"
          value={selected.difficultyAtMost == null ? "" : String(selected.difficultyAtMost)}
          onChange={e => onPatch({ difficultyAtMost: num(e.target.value) })}
          options={DIFFICULTIES} />
        <span style={hint}>
          One scenario can be three. The difficulty it is played at is set on the
          Teams tab.
        </span>
      </div>
    </div>
  );
}
