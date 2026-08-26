import React from "react";
import { Button, Input, Select, IconButton, EmptyState } from "../ds/shiro.js";
import { label, hint, TimeField, CountField, UnitPicker, clock } from "./parts.jsx";

/* Authoring the objectives the game actually evaluates.
 *
 * Zero-K checks unit counts over time windows: 24 fields whose useful
 * combinations are not guessable from their names, and one of which is spelled
 * `comparisionType`. Exposing that table would be exposing a trap, so the
 * author picks one of seven things people actually mean and `scenario.rs`
 * compiles it.
 *
 * This is the surface that did not exist. The data model has been in the Rust
 * for a while and nothing could reach it: the editor collected sentences, and
 * sentences are not evaluated by anything. */

/** The seven goals, and what each one needs the author to fill in. */
export const GOALS = [
  {
    kind: "surviveUntil",
    title: "Survive until",
    blurb: "Keep at least one of these alive to the deadline.",
    make: () => ({ kind: "surviveUntil", seconds: 300, units: [] }),
  },
  {
    kind: "buildBy",
    title: "Build n by",
    blurb: "Produce this many before the clock. Ones that died on the way count.",
    make: unit => ({ kind: "buildBy", unit, count: 5, seconds: 300 }),
  },
  {
    kind: "haveAtOnce",
    title: "Have n at once",
    blurb: "Hold this many at one moment. Losses are not forgiven.",
    make: unit => ({ kind: "haveAtOnce", unit, count: 5 }),
  },
  {
    kind: "destroyAllBy",
    title: "Destroy all by",
    blurb: "None of the enemy's left by the deadline.",
    make: unit => ({ kind: "destroyAllBy", unit, seconds: 300 }),
  },
  {
    kind: "killCount",
    title: "Kill n",
    blurb: "Kill this many, however long it takes.",
    make: unit => ({ kind: "killCount", unit, count: 3 }),
  },
  {
    kind: "winBefore",
    title: "Win before",
    blurb: "Win the match before the clock runs out.",
    make: () => ({ kind: "winBefore", seconds: 600 }),
  },
];

/** A sentence describing what the game will check, for the list. */
export function describe(goal) {
  switch (goal.kind) {
    case "surviveUntil":
      return goal.units.length
        ? `Keep ${goal.units.join(", ")} alive until ${clock(goal.seconds)}`
        : `Survive until ${clock(goal.seconds)}`;
    case "buildBy":
      return `Build ${goal.count} × ${goal.unit} by ${clock(goal.seconds)}`;
    case "haveAtOnce":
      return `Have ${goal.count} × ${goal.unit} at once`;
    case "destroyAllBy":
      return `Destroy all ${goal.unit} by ${clock(goal.seconds)}`;
    case "killCount":
      return `Kill ${goal.count} × ${goal.unit}`;
    case "winBefore":
      return `Win before ${clock(goal.seconds)}`;
    default:
      return goal.kind;
  }
}

/** The fields one goal needs, which differ per kind. */
function GoalFields({ goal, onChange, roster }) {
  const set = patch => onChange({ ...goal, ...patch });
  /* minWidth 0 on the children: a grid track is `auto` at minimum, so an
     Input with its own intrinsic width pushes the second column past the edge
     of a 320px panel and the deadline gets clipped. */
  const row = {
    display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--sp-4)",
  };
  const cell = { minWidth: 0 };

  switch (goal.kind) {
    case "surviveUntil":
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          <TimeField label="Until" value={goal.seconds} onChange={s => set({ seconds: s })} />
          <UnitPicker
            label="Unit that must survive"
            roster={roster}
            value={goal.units[0] ?? ""}
            onChange={u => set({ units: u ? [u] : [] })}
          />
          <span style={hint}>
            Leave the unit empty and the objective holds as long as you have anything at all.
          </span>
        </div>
      );
    case "buildBy":
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          <UnitPicker label="Unit" roster={roster} value={goal.unit} onChange={u => set({ unit: u })} />
          <div style={row}>
            <div style={cell}>
              <CountField label="How many" value={goal.count} onChange={c => set({ count: c })} />
            </div>
            <div style={cell}>
              <TimeField label="By" value={goal.seconds} onChange={s => set({ seconds: s })} />
            </div>
          </div>
        </div>
      );
    case "haveAtOnce":
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          <UnitPicker label="Unit" roster={roster} value={goal.unit} onChange={u => set({ unit: u })} />
          <CountField label="How many at once" value={goal.count} onChange={c => set({ count: c })} />
        </div>
      );
    case "destroyAllBy":
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          <UnitPicker label="Enemy unit" roster={roster} value={goal.unit} onChange={u => set({ unit: u })} />
          <TimeField label="By" value={goal.seconds} onChange={s => set({ seconds: s })} />
        </div>
      );
    case "killCount":
      return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
          <UnitPicker label="Enemy unit" roster={roster} value={goal.unit} onChange={u => set({ unit: u })} />
          <CountField label="How many" value={goal.count} onChange={c => set({ count: c })} />
        </div>
      );
    case "winBefore":
      return <TimeField label="Win before" value={goal.seconds} onChange={s => set({ seconds: s })} />;
    default:
      return null;
  }
}

export default function Objectives({ goals, setGoals, notes, setNotes, roster }) {
  const [note, setNote] = React.useState("");
  const [open, setOpen] = React.useState(null);
  const firstUnit = roster[0]?.name ?? "";

  const add = kind => {
    const spec = GOALS.find(g => g.kind === kind);
    setGoals(v => [...v, { description: spec.title, goal: spec.make(firstUnit) }]);
    setOpen(goals.length);
  };

  const patch = (i, next) => setGoals(v => v.map((o, j) => (j === i ? next : o)));

  return (
    <div style={{ padding: "var(--sp-5)", display: "flex", flexDirection: "column",
      gap: "var(--sp-5)" }}>
      <div>
        <span style={label}>Checked objectives</span>
        <span style={{ ...hint, display: "block", marginTop: "var(--sp-2)" }}>
          The game evaluates these and shows them in the objective panel.
        </span>
      </div>

      {goals.length === 0 && (
        <EmptyState icon="target" title="No objectives yet."
          body="Add one below. A scenario with none can still be played, but nothing marks it won." />
      )}

      {goals.map((objective, i) => (
        <div key={i} style={{ border: "1px solid var(--w-12)", background: "var(--surface-base)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-4)",
            padding: "var(--sp-4)" }}>
            <button type="button" onClick={() => setOpen(open === i ? null : i)}
              style={{ flex: 1, textAlign: "left", background: "none", border: 0, padding: 0,
                cursor: "pointer", color: "inherit", font: "inherit", minWidth: 0 }}>
              <span style={{ font: "var(--text-ui-sm)", color: "var(--text-hi)", display: "block",
                overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {objective.description || "(no description)"}
              </span>
              <span style={{ ...hint, display: "block" }}>{describe(objective.goal)}</span>
            </button>
            <IconButton icon={open === i ? "chevron-up" : "chevron-down"} label="Edit" size="sm"
              onClick={() => setOpen(open === i ? null : i)} />
            <IconButton icon="x" label="Remove" size="sm"
              onClick={() => { setGoals(v => v.filter((_, j) => j !== i)); setOpen(null); }} />
          </div>

          {open === i && (
            <div style={{ padding: "var(--sp-5)", borderTop: "1px solid var(--w-06)",
              display: "flex", flexDirection: "column", gap: "var(--sp-4)" }}>
              <Input label="What the player is told" size="sm" value={objective.description}
                onChange={e => patch(i, { ...objective, description: e.target.value })} />
              <Select label="Goal" size="sm" value={objective.goal.kind}
                onChange={e => {
                  const spec = GOALS.find(g => g.kind === e.target.value);
                  patch(i, { ...objective, goal: spec.make(firstUnit) });
                }}
                options={GOALS.map(g => ({ value: g.kind, label: g.title }))} />
              <span style={hint}>{GOALS.find(g => g.kind === objective.goal.kind)?.blurb}</span>
              <GoalFields goal={objective.goal} roster={roster}
                onChange={goal => patch(i, { ...objective, goal })} />
            </div>
          )}
        </div>
      ))}

      <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--sp-3)" }}>
        {GOALS.map(g => (
          <Button key={g.kind} variant="secondary" size="sm" onClick={() => add(g.kind)}>
            {g.title}
          </Button>
        ))}
      </div>

      <div style={{ borderTop: "1px solid var(--w-06)", paddingTop: "var(--sp-5)" }}>
        <span style={label}>Notes for the briefing</span>
        <span style={{ ...hint, display: "block", margin: "var(--sp-2) 0 var(--sp-4)" }}>
          Sentences the game shows before the match but does not check. Not every
          intention is a unit count.
        </span>
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--sp-3)" }}>
          {notes.map((o, i) => (
            <div key={i} style={{ display: "flex", alignItems: "flex-start", gap: "var(--sp-4)",
              padding: "var(--sp-4)", background: "var(--surface-base)",
              border: "1px solid var(--w-12)" }}>
              <span style={{ flex: 1, font: "var(--text-ui-sm)", color: "var(--text-body)" }}>{o}</span>
              <IconButton icon="x" label="Remove" size="sm"
                onClick={() => setNotes(v => v.filter((_, j) => j !== i))} />
            </div>
          ))}
          <Input size="sm" placeholder="Add a note, then press Enter" value={note}
            onChange={e => setNote(e.target.value)}
            onKeyDown={e => {
              if (e.key === "Enter" && note.trim()) {
                setNotes(v => [...v, note.trim()]);
                setNote("");
              }
            }} />
        </div>
      </div>
    </div>
  );
}
