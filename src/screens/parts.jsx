import React from "react";
import { Input, Select } from "../ds/shiro.js";

/* Small pieces the editor panels share. */

export const label = {
  font: "var(--text-label)", letterSpacing: "var(--track-label)",
  textTransform: "uppercase", color: "var(--text-faint)",
};

export const hint = {
  font: "var(--w-regular) var(--size-micro)/1.5 var(--font-core)",
  color: "var(--text-faint)",
};

export const mono = {
  font: "var(--w-regular) var(--size-tiny)/1.4 var(--font-mono)",
  color: "var(--text-body)",
};

/** `95` -> `1:35`. Objective deadlines are minutes and seconds to a player. */
export function clock(seconds) {
  const s = Math.max(0, Math.round(Number(seconds) || 0));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** `1:35` or `95` -> `95`. Accepts either, because both get typed. */
export function seconds(text) {
  const raw = String(text).trim();
  if (!raw) return 0;
  const parts = raw.split(":");
  if (parts.length === 2) {
    return Math.max(0, (Number(parts[0]) || 0) * 60 + (Number(parts[1]) || 0));
  }
  return Math.max(0, Math.round(Number(raw) || 0));
}

/**
 * A time, typed the way a player reads it.
 *
 * Kept as text while it is being edited so that clearing the field does not
 * snap back to `0:00` under the cursor, which makes it impossible to type
 * `12:00` starting from `1:00`.
 */
export function TimeField({ label: text, value, onChange, size = "sm" }) {
  const [draft, setDraft] = React.useState(null);
  return (
    <Input
      label={text}
      size={size}
      value={draft ?? clock(value)}
      onChange={e => {
        setDraft(e.target.value);
        onChange(seconds(e.target.value));
      }}
      onBlur={() => setDraft(null)}
      placeholder="m:ss"
    />
  );
}

/** A whole number, floored at `min`. */
export function CountField({ label: text, value, onChange, min = 1, size = "sm" }) {
  return (
    <Input
      label={text}
      size={size}
      type="number"
      min={min}
      value={String(value)}
      onChange={e => onChange(Math.max(min, Math.round(Number(e.target.value) || min)))}
    />
  );
}

/**
 * Pick a unit by what it is called, and store what the engine calls it.
 *
 * The two differ for almost every unit in Zero-K - a player places a Glaive and
 * the script has to say `cloakraid` - so showing only one of them makes the
 * editor either unreadable or unusable. Both are shown; the internal name is
 * what travels.
 */
export function UnitPicker({ label: text, value, onChange, roster, size = "sm" }) {
  const options = React.useMemo(() => {
    const seen = new Set();
    const out = [];
    for (const u of roster) {
      if (seen.has(u.name)) continue;
      seen.add(u.name);
      out.push({ value: u.name, label: `${u.title} — ${u.name}` });
    }
    // A unit the roster does not know about is still shown, so opening a
    // scenario written against a different Zero-K does not silently retarget
    // its objectives at the first unit in the list.
    if (value && !seen.has(value)) {
      out.unshift({ value, label: `${value} (not in this roster)` });
    }
    return out;
  }, [roster, value]);

  return (
    <Select
      label={text}
      size={size}
      value={value ?? ""}
      onChange={e => onChange(e.target.value)}
      options={options}
    />
  );
}
