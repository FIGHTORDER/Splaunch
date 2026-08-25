import React from "react";
import { Button, Select, Input, IconButton, Checkbox } from "../ds/shiro.js";
import { label, hint, TimeField, UnitPicker } from "./parts.jsx";

/* Teams, sides, and what losing means.
 *
 * The editor used to hard-code exactly two teams - you in blue, one NullAI in
 * red - which is not a scenario editor so much as a duel editor. Zero-K's
 * campaign is full of three-way maps, allied AI wingmen and neutral Gaia
 * defenders, and all of it is expressible in the same start script. */

/** The game's own team colours, so an author thinking "red team" gets red. */
export const TEAM_COLOURS = [
  { rgb: "0 0 1", css: "#3b6cf5", name: "Blue" },
  { rgb: "1 0 0", css: "#e0403a", name: "Red" },
  { rgb: "0 0.7 0.2", css: "#28b45a", name: "Green" },
  { rgb: "1 0.8 0", css: "#e8b21c", name: "Yellow" },
  { rgb: "0.6 0.2 0.8", css: "#9b46c9", name: "Purple" },
  { rgb: "1 0.5 0", css: "#ee7d1b", name: "Orange" },
];

export function colourOf(teamId) {
  return TEAM_COLOURS[teamId % TEAM_COLOURS.length];
}

export default function Teams({
  teams, setTeams, defeat, setDefeat, ais, roster, units, difficulty, setDifficulty,
}) {
  const addTeam = () => {
    const id = teams.length ? Math.max(...teams.map(t => t.id)) + 1 : 0;
    setTeams(v => [...v, { id, ally: id, ai: ais[0] ?? "NullAI", colour: colourOf(id).rgb }]);
  };

  const patch = (id, next) => setTeams(v => v.map(t => (t.id === id ? { ...t, ...next } : t)));

  const allies = [...new Set(teams.map(t => t.ally))].sort((a, b) => a - b);
  const defeatFor = ally => defeat.find(d => d.ally === ally);
  const patchDefeat = (ally, next) =>
    setDefeat(v => {
      const existing = v.find(d => d.ally === ally);
      if (!existing) return [...v, { ally, vitalUnits: [], loseAfterSeconds: null, ...next }];
      return v.map(d => (d.ally === ally ? { ...d, ...next } : d));
    });

  return (
    <div style={{ padding: "var(--sp-5)", display: "flex", flexDirection: "column",
      gap: "var(--sp-5)" }}>
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
        <span style={label}>Difficulty</span>
        <Select size="sm" value={String(difficulty)}
          onChange={e => setDifficulty(Number(e.target.value))}
          options={[
            { value: "1", label: "Easy" },
            { value: "2", label: "Normal" },
            { value: "3", label: "Hard" },
          ]} />
        <span style={hint}>
          Units can be set to exist only at some difficulties, on the Selection
          tab, so one scenario can be three.
        </span>
      </div>

      <div style={{ borderTop: "1px solid var(--w-06)", paddingTop: "var(--sp-5)" }}>
        <span style={label}>Teams</span>
        <span style={{ ...hint, display: "block", marginTop: "var(--sp-2)" }}>
          One of them has to be you. Teams sharing a side are allies.
        </span>
      </div>

      {teams.map(team => (
        <div key={team.id} style={{ border: "1px solid var(--w-12)", padding: "var(--sp-4)",
          background: "var(--surface-base)", display: "flex", flexDirection: "column",
          gap: "var(--sp-4)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-4)" }}>
            <span style={{ width: 10, height: 10, background: colourOf(team.id).css }} />
            <span style={{ font: "var(--text-ui-sm)", color: "var(--text-hi)", flex: 1 }}>
              {colourOf(team.id).name}{team.ai ? "" : " (you)"}
            </span>
            {teams.length > 1 && (
              <IconButton icon="x" label="Remove team" size="sm" onClick={() => {
                setTeams(v => v.filter(t => t.id !== team.id));
              }} />
            )}
          </div>
          <Checkbox
            label="Played by you"
            checked={team.ai === null}
            onChange={e => patch(team.id, { ai: e.target.checked ? null : (ais[0] ?? "NullAI") })}
          />
          {team.ai !== null && (
            <Select label="AI" size="sm" value={team.ai}
              onChange={e => patch(team.id, { ai: e.target.value })}
              options={(ais.length ? ais : [team.ai]).map(a => ({ value: a, label: a }))} />
          )}
          <Input label="Side" size="sm" type="number" min={0} value={String(team.ally)}
            onChange={e => patch(team.id, { ally: Math.max(0, Number(e.target.value) || 0) })} />
        </div>
      ))}

      <Button variant="secondary" size="sm" icon="plus" onClick={addTeam}>Add a team</Button>

      <div style={{ borderTop: "1px solid var(--w-06)", paddingTop: "var(--sp-5)" }}>
        <span style={label}>Losing</span>
        <span style={{ ...hint, display: "block", margin: "var(--sp-2) 0 var(--sp-4)" }}>
          Without these a side is only beaten when it has nothing left at all,
          which is a long way to lose a mission that was about one commander.
        </span>
        {allies.map(ally => {
          const d = defeatFor(ally);
          return (
            <div key={ally} style={{ border: "1px solid var(--w-12)", padding: "var(--sp-4)",
              marginBottom: "var(--sp-4)", display: "flex", flexDirection: "column",
              gap: "var(--sp-4)", background: "var(--surface-base)" }}>
              <span style={{ font: "var(--text-ui-sm)", color: "var(--text-hi)" }}>Side {ally}</span>
              <UnitPicker
                label="Loses when this is gone"
                roster={roster}
                value={d?.vitalUnits?.[0] ?? ""}
                onChange={u => patchDefeat(ally, { vitalUnits: u ? [u] : [] })}
              />
              <Checkbox
                label="Also lose on a timer"
                checked={d?.loseAfterSeconds != null}
                onChange={e => patchDefeat(ally, { loseAfterSeconds: e.target.checked ? 600 : null })}
              />
              {d?.loseAfterSeconds != null && (
                <TimeField label="Lose after" value={d.loseAfterSeconds}
                  onChange={s => patchDefeat(ally, { loseAfterSeconds: s })} />
              )}
            </div>
          );
        })}
      </div>

      <span style={hint}>
        {units.length} unit{units.length === 1 ? "" : "s"} placed across {teams.length} team
        {teams.length === 1 ? "" : "s"}.
      </span>
    </div>
  );
}
