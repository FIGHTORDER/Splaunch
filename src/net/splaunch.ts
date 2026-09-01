/**
 * Splaunch's backend, from the front.
 *
 * A scenario compiles to a Spring start script in Rust and launches the real
 * game, so there is no preview to keep in sync - see
 * `src-tauri/src/scenario.rs`.
 *
 * These types mirror `scenario.rs`. When one changes the other has to, and the
 * cost of them disagreeing is a field that serialises to nothing and an
 * objective that quietly never fires.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { inTauri } from "./connection.ts";

/** A unit on the map. Everything past `z` is optional and omitted when unset. */
export interface Placed {
  unit: string;
  team: number;
  x: number;
  z: number;
  facing?: number | null;
  buildProgress?: number | null;
  experience?: number | null;
  movestate?: number | null;
  invincible?: boolean | null;
  terraformHeight?: number | null;
  /** Owned by Gaia rather than by a team. */
  neutral?: boolean;
  /** Points to walk, in order, for ever. Needs at least two to be a route. */
  patrol?: [number, number][];
  /** Patrol on the spot, facing the middle of the map. */
  selfPatrol?: boolean;
  difficultyAtLeast?: number | null;
  difficultyAtMost?: number | null;
}

/** A label on the map, shown to the player from the start. */
export interface Marker { x: number; z: number; text: string }

export interface ScenarioTeam { id: number; ally: number; ai: string | null; colour: string }

export interface Feature { name: string; x: number; z: number; facing?: number | null }

/**
 * What an author means, rather than the 24 fields it takes.
 *
 * The `kind` discriminant matches serde's tag on `Goal`, so these strings are
 * load-bearing rather than labels.
 */
export type Goal =
  | { kind: "surviveUntil"; seconds: number; units: string[] }
  | { kind: "buildBy"; unit: string; count: number; seconds: number }
  | { kind: "haveAtOnce"; unit: string; count: number }
  | { kind: "destroyAllBy"; unit: string; seconds: number }
  | { kind: "killCount"; unit: string; count: number }
  | { kind: "winBefore"; seconds: number };

export interface Objective { description: string; goal: Goal }

export interface Defeat { ally: number; vitalUnits: string[]; loseAfterSeconds?: number | null }

export interface Scenario {
  formatVersion: number;
  name: string;
  map: string;
  /** The archive name `GameType` carries, e.g. "Zero-K v1.14.8.0". */
  game: string;
  teams: ScenarioTeam[];
  units: Placed[];
  /** Notes for the briefing. Not evaluated - see `goals` for that. */
  objectives: string[];
  goals: Objective[];
  features: Feature[];
  briefing: string | null;
  defeat: Defeat[];
  /** The map's width in elmos - the x axis. */
  mapElmos: number;
  /**
   * The map's depth in elmos - the z axis - where it differs from the width.
   * Null means square. 145 of the catalogue's 343 maps are not square, and
   * one figure for both axes put every unit on those at the wrong depth.
   */
  mapElmosZ: number | null;
  /**
   * Modoptions the battle carries besides the mission's own - where a Coilbox
   * preset's `modOptionValues` land. Written before the mission's keys, so a
   * preset cannot overwrite the mission engine.
   */
  modOptions: Record<string, string>;
  markers: Marker[];
  /** 1 easy, 2 normal, 3 hard. Gates the per-unit difficulty fields. */
  difficulty: number;
}

export const FORMAT_VERSION = 1;
export const DEFAULT_MAP_ELMOS = 8 * 512;
/** Zero-K's own default, from mission_galaxy_campaign_battle.lua. */
export const DEFAULT_DIFFICULTY = 2;

export interface CatalogueMap {
  name: string;
  resourceId: number;
  widthElmos: number | null;
  heightElmos: number | null;
}

/** Every map Zero-K's content service knows about. */
export async function maps(): Promise<CatalogueMap[]> {
  if (!inTauri()) return [];
  return invoke<CatalogueMap[]>("sp_maps");
}

export interface Preview {
  install: { root: string; source: string };
  exe: string;
  /** The engine version a launch would actually use. */
  engine: string;
  /** The archive name `GameType` would carry, if one was found. */
  game: string | null;
}

/** Where Zero-K is, and which engine a scenario would run on. */
export async function launchPreview(engine = ""): Promise<Preview> {
  if (!inTauri()) throw new Error("Splaunch needs the desktop app.");
  return invoke<Preview>("sp_launch_preview", { engine });
}

export interface GameArchive { name: string; path: string }
export interface GameInfo {
  engines: string[];
  games: GameArchive[];
  ais: string[];
  /** Maps actually on this machine, as opposed to the 343 in the catalogue. */
  maps: string[];
  engine: string | null;
  game: string | null;
}

/**
 * Whether a catalogue name matches an installed archive.
 *
 * The catalogue says "Comet Catcher Redux" and the file on disk is
 * `comet_catcher_redux.sd7`, so neither case nor the spaces can be trusted.
 * Mirrors `game::map_is_installed`.
 */
export function mapIsInstalled(installed: string[], name: string): boolean {
  const normal = (s: string) => s.toLowerCase().replace(/[ _-]/g, "");
  const wanted = normal(name);
  return installed.some(m => normal(m) === wanted);
}

/** What the install contains: engines, game archives, AIs. */
export async function gameInfo(): Promise<GameInfo> {
  if (!inTauri()) throw new Error("Splaunch needs the desktop app.");
  return invoke<GameInfo>("sp_game_info");
}

export interface UnitDef {
  name: string;
  title: string;
  description: string;
  group: string;
  /** The file in `unitpics/` that draws it. Not `<name>.png` for 29 of them. */
  buildPic?: string;
}
export interface Roster { source: string; units: UnitDef[] }

/**
 * The placeable roster, from the installed game where possible.
 *
 * Never throws: an editor with no unit list is not usable, so the backend falls
 * back to its vendored copy and reports which one answered.
 */
export async function units(): Promise<Roster> {
  if (!inTauri()) return { source: "", units: [] };
  return invoke<Roster>("sp_units");
}

/**
 * Pictures for the named units, as `data:` URIs.
 *
 * Asked for by name rather than fetched wholesale: the archive holds 366 and a
 * scenario places a dozen. A unit with no picture is absent from the result
 * rather than an error - a missing icon should not become a missing unit.
 */
export async function unitIcons(names: string[]): Promise<Record<string, string>> {
  if (!inTauri() || !names.length) return {};
  return invoke<Record<string, string>>("sp_unit_icons", { names });
}

/** A decoded image, ready for `putImageData`. */
export interface Rgba { width: number; height: number; pixels: string }

/**
 * Zoom-out icons for the named units, decoded to RGBA.
 *
 * These are the flat silhouettes the game draws when you zoom out, and they
 * are what stays readable at the size a map marker actually is - a build
 * picture is a lit 3D render that turns to mush at twenty pixels.
 */
export async function unitMarks(names: string[]): Promise<Record<string, Rgba>> {
  if (!inTauri() || !names.length) return {};
  return invoke<Record<string, Rgba>>("sp_unit_marks", { names });
}

/** A map's metal infomap: one byte a sample, sixteen elmos to a sample. */
export interface MetalMap { width: number; height: number; samples: string }

/**
 * Where the metal is on a map, read out of its own archive.
 *
 * Density rather than a list of spots, deliberately: what counts as "a spot" is
 * a choice, and two reasonable choices give two different answers from the same
 * map. Coilbox pins those numbers in a shared catalogue so clients agree, and
 * inventing a second definition here is exactly the divergence that exists to
 * prevent. See `src-tauri/src/mapfile.rs`.
 *
 * `null` when the map is not installed or its archive cannot be read.
 */
export async function mapMetal(map: string): Promise<MetalMap | null> {
  if (!inTauri() || !map) return null;
  return invoke<MetalMap | null>("sp_map_metal", { map });
}

/** The script this scenario would produce, without launching it. */
export async function scenarioScript(scenario: Scenario, player: string): Promise<string> {
  if (!inTauri()) return "";
  return invoke<string>("spsc_script", { scenario, player });
}

/**
 * What is wrong with it, in sentences.
 *
 * The editor asks rather than deciding for itself. It used to keep its own
 * list, which drifted from the one that actually gates `write_script` - so a
 * scenario could pass every check the author could see and still be refused.
 */
/** A Coilbox preset laid over a scenario, and what could not come with it. */
export interface AppliedPreset {
  scenario: Scenario;
  /** Shown to the author. A preset that plays differently here is the one
   *  thing the format exists to prevent, so losses are never silent. */
  ignored: string[];
  name: string;
}

/**
 * Open a Coilbox preset and lay it over this scenario.
 *
 * `text` is for a pasted `cbz1.` share code; without one the backend opens a
 * file picker. Null means the author closed the dialog.
 */
export async function importPreset(
  scenario: Scenario, text?: string,
): Promise<AppliedPreset | null> {
  if (!inTauri()) return null;
  return invoke<AppliedPreset | null>("spsc_import_preset", { scenario, text: text ?? null });
}

/** Write the battle half of this scenario out as a Coilbox preset. */
export async function exportPreset(scenario: Scenario): Promise<string | null> {
  if (!inTauri()) return null;
  return invoke<string | null>("spsc_export_preset", { scenario });
}

/** What an author should know but that will not stop a launch. */
export async function scenarioWarnings(scenario: Scenario): Promise<string[]> {
  if (!inTauri()) return [];
  return invoke<string[]>("spsc_warnings", { scenario });
}

export async function scenarioProblems(scenario: Scenario): Promise<string[]> {
  if (!inTauri()) return [];
  return invoke<string[]>("spsc_problems", { scenario });
}

/** Compile and launch the game into it. Resolves with the engine's pid. */
export async function testScenario(
  scenario: Scenario, player: string, engine: string,
): Promise<number> {
  if (!inTauri()) throw new Error("Testing a scenario needs the desktop app.");
  return invoke<number>("spsc_test", { scenario, player, engine });
}

/** Save it, asking where. Resolves to `null` if the dialog was dismissed. */
export async function saveScenario(scenario: Scenario): Promise<string | null> {
  if (!inTauri()) throw new Error("Saving needs the desktop app.");
  return invoke<string | null>("spsc_save", { scenario });
}

/**
 * Pack this scenario as a one-mission campaign, asking where to write it.
 *
 * A campaign of one is a real campaign, and this is the path from "I made a
 * mission" to "somebody else can play it": the mission ships compiled, with the
 * three values that cannot travel between machines written as markers for the
 * loader to fill in. `src-tauri/src/campaign.rs` has the format.
 *
 * Resolves to `null` if the dialog was dismissed, which is not an error.
 */
export async function exportCampaign(
  scenario: Scenario, author: string,
): Promise<string | null> {
  if (!inTauri()) throw new Error("Exporting needs the desktop app.");
  return invoke<string | null>("spsc_export_campaign", { scenario, author });
}

/** Open one, asking which. Resolves to `null` if the dialog was dismissed. */
export async function openScenario(): Promise<Scenario | null> {
  if (!inTauri()) throw new Error("Opening needs the desktop app.");
  return invoke<Scenario | null>("spsc_open");
}

/** The scenario Splaunch ships with, bundled into the binary. */
export async function exampleScenario(): Promise<Scenario> {
  if (!inTauri()) throw new Error("Opening needs the desktop app.");
  return invoke<Scenario>("spsc_example");
}

/** What the engine is doing. */
export type GameStatus =
  | { kind: "launched"; pid: number }
  | { kind: "exited"; code: number | null }
  | { kind: "failed"; reason: string };

/**
 * Listen for the engine starting and stopping.
 *
 * Nothing used to listen for this. The backend has always announced the exit,
 * and the editor cleared `running` only when the launch itself threw - so after
 * one successful test the app believed a game was running for the rest of its
 * life, and refused to start another.
 */
export function onGame(cb: (s: GameStatus) => void): Promise<UnlistenFn> {
  return listen<GameStatus>("splaunch://game", e => cb(e.payload));
}
