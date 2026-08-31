# Staying compatible with Coilbox

Written 2026-08-31 from Coilbox at `20cbdf4d64e6`, after its author set out how
the layers fit together and asked that implementations not diverge. This records
the contract as measured from the source, so the next person does not re-derive
it. It is not a plan - see the end for what is undecided.

## The chain, bottom to top

| Layer | What it adds | Coilbox `kind` | `kindVersion` |
|---|---|---|---|
| Preset | Sets up a battle: game, map, opponents, modoptions, restrictions | `preset` | 1 |
| Scenario | Rules, events, dialogue inside that battle | `scenario` | 2 |
| Mission | Wraps a scenario, adds briefing and artwork | (in `campaign`) | - |
| Campaign | Ties missions together | `campaign` | 2 |

The author's advice is to work bottom up, because Coilbox can then author each
layer and act as the reference implementation. The hub carries presets only for
now - the layers above need media bundling (video, voice) that it does not do
yet.

## The container, which is the actual contract

Every shared artefact is one envelope (`src/container/container.ts`):

```ts
export interface Container<P = unknown> {
  format: typeof CONTAINER_FORMAT;   // "coilbox"
  container: typeof CONTAINER_VERSION; // 1
  kind: ContainerKind;
  kindVersion: number;
  payload: P;
}
```

`kind` is one of `campaign`, `preset`, `challenge`, `setup-pack`, `scenario`,
`keymap`, `blueprint`.

Two independent version numbers rather than semver, deliberately: a payload
whose `container` or `kindVersion` is higher than the build supports is reported
as `"newer"` rather than half-read. `identify()` answers kind, version and
compatibility without validating the payload, so a reader can refuse politely.

Additive fields need no `kindVersion` bump, because older builds ignore keys they
do not know. That is the extensibility point, and it is the thing to respect.

Shareable codes are DEFLATE, then base64url, prefixed `cbz1.`
(`COMPRESSED_CODE_PREFIX`). `decodeContainerText()` accepts raw JSON, that code,
and a legacy uncompressed base64url form.

## The preset payload

From `src/play/presets.ts`, `PRESET_KIND_VERSION = 1`:

```ts
export interface SkirmishPreset extends SkirmishDraft {
  id: string;
  name: string;
  createdAt: string;
  lastUsedAt: string;
}
export interface PresetPayload extends SkirmishPreset {
  game?: GameIdentity;   // additive, hence optional
}
```

The draft underneath carries `participants[]`, `gameName`, `mapName`,
`startPosType`, `modOptionValues`, and an optional `restrictions`
(`disabledUnits: string[]`, `advantage: number`, `incomeMultiplier: number`).
Import validates those types and drops malformed `restrictions` entries rather
than failing the whole file.

**This maps onto Splaunch almost exactly.** A preset is the half of a
`Scenario` that is not scenario-specific: `map`, `game`, and `teams` (id, ally,
ai, colour) plus the modoptions and `StartposType` that `write_script` already
emits. That is why it is the cheap first step.

## Where Splaunch and Coilbox genuinely differ

Not a formatting difference, and worth stating before anyone promises
compatibility.

- **Coilbox** compiles a scenario to a `mission.lua` and installs a versioned
  runtime gadget into a loose `.sdd` game folder. Engine-level, and it works for
  any Spring or Recoil game.
- **Splaunch** installs nothing. It arms Zero-K's *own* campaign gadget -
  `mission_galaxy_campaign_battle.lua`, already in the base game - through
  modoptions carrying base64 payloads. That is the whole reason it is small, and
  it is Zero-K-only by construction.

So "keep the missions and the mission runtime compatible" is cheap at the
container and payload level and expensive at the runtime level. The author says
he cannot speak to how his runtime interacts with the ZK code, which is the same
seam from the other side.

## A warning that already applies to shipped code

Coilbox's author reports that listing and finding content through the existing
game infrastructure triggered backfill processes at BAR, with real consequences
including a dead API endpoint, and that he now self-hosts that data on the hub
instead.

Splaunch does the same shape of thing today:

- `maps.rs` posts `GetPublicCommunityInfo` to `zero-k.info/ContentService.svc`
  on every start and pulls the whole 343-map catalogue.
- The map picker asks `zero-k.info/Resources/<name>.minimap.jpg` for every card
  it draws, up to 48 at once, and the editor asks for one more per map opened.

Neither is cached across runs. Worth fixing on its own merits, independently of
anything Coilbox-shaped: cache the catalogue on disk with an age check, and load
minimaps lazily. See also `src/screens/Minimap.jsx`, which documents why those
images are the wrong shape anyway - reading `.smf` (vendored here, item 1) would
remove the need for most of these requests.
