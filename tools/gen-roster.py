#!/usr/bin/env python3
"""Vendor Zero-K's unit roster, so the editor has real unit names offline.

The authority is always the installed game: `game.rs` reads `units/*.lua` out
of `zk-stable.sdz` and that wins whenever an install is present. This file is
the fallback for a machine that has no Zero-K yet, and for the map picker
before an install has been located.

It exists because the list it replaced was invented. Splaunch shipped a
hand-written palette of *Balanced Annihilation* names - `armpw`, `corhlt`,
`armmex` - and not one of the twenty-three is a unit Zero-K defines, so every
scenario built with it placed nothing at all.

Usage:  python3 tools/gen-roster.py <path-to-Zero-K-checkout>
Writes: src-tauri/src/roster.json, and prints the commit to pin beside it.
"""
import json, os, re, subprocess, sys

def field(src, key):
    m = re.search(r'\b%s\s*=\s*\[\[(.*?)\]\]' % re.escape(key), src, re.S)
    if m: return m.group(1).strip()
    m = re.search(r'\b%s\s*=\s*"(.*?)"' % re.escape(key), src, re.S)
    if m: return m.group(1).strip()
    m = re.search(r"\b%s\s*=\s*'(.*?)'" % re.escape(key), src, re.S)
    return m.group(1).strip() if m else ""

def main(root):
    unit_dir = os.path.join(root, "units")
    if not os.path.isdir(unit_dir):
        sys.exit("no units/ under %s" % root)

    units, builders = {}, {}
    for name in sorted(os.listdir(unit_dir)):
        if not name.endswith(".lua"):
            continue
        src = open(os.path.join(unit_dir, name), encoding="utf-8", errors="replace").read()
        # The internal name is the table key, not the filename: 274 of 275
        # agree, and damagesinkrock.lua defines `rocksink`.
        m = re.search(r'return\s*\{\s*([A-Za-z0-9_]+)\s*=', src)
        key = m.group(1) if m else name[:-4]
        entry = {
            "name": key,
            "title": field(src, "name") or key,
            "description": field(src, "description"),
            "group": "Other",
        }
        # Not <name>.png: 29 units disagree with that guess and 11 declare
        # nothing, so the wrong picture would end up on the wrong unit.
        pic = field(src, "buildPic")
        if pic:
            entry["buildPic"] = pic
        # The zoom-out icon, which is shared: 275 units draw from 204 types.
        icon = field(src, "iconType")
        if icon:
            entry["iconType"] = icon
        units[key] = entry
        opts = re.search(r'buildoptions\s*=\s*\{(.*?)\}', src, re.S)
        if opts:
            built = re.findall(r'\[\[(.*?)\]\]', opts.group(1))
            if built:
                builders[key] = built

    # Group by what builds a unit, which is the game's own classification.
    #
    # Factories rank first, then their plates, then everything else. That order
    # is load-bearing rather than tidy: `athena` builds a 22-unit cross-section
    # drawn from six different factories, so ranking it alphabetically lets it
    # absorb six of the Cloakbot Factory's eleven units and leaves the group a
    # player knows by name with five. A factory and its plate build the same
    # list, and first writer wins.
    def rank(b):
        return (0 if b.startswith("factory") else 1 if b.startswith("plate") else 2,
                units.get(b, {}).get("title", b))

    for builder in sorted(builders, key=rank):
        label = units.get(builder, {}).get("title", builder)
        for built in builders[builder]:
            if built in units and units[built]["group"] == "Other":
                units[built]["group"] = label

    # What no builder claims still has to be findable, and it is the half of
    # the roster a scenario most wants: commanders, turrets, economy. Zero-K
    # names these systematically, so the prefixes group them. Unlike the rule
    # above this taxonomy is ours rather than the game's, which is why it runs
    # second and only over what is left.
    BY_NAME = [
        (r"^(factory|plate)", "Factories"),
        (r"^turret", "Defences"),
        (r"^energy", "Economy"),
        (r"^static", "Support Structures"),
        (r"com", "Commanders"),
        (r"^dyn.*\d$", "Commanders"),
        (r"^chicken", "Chickens"),
        (r"^strider", "Striders"),
        (r"^(dbg_|fakeunit|tiptest|empiricaldps|damagesink|rocksink)", "Test and debug"),
    ]
    for unit in units.values():
        if unit["group"] != "Other":
            continue
        for pattern, label in BY_NAME:
            if re.search(pattern, unit["name"]):
                unit["group"] = label
                break

    # "Other" and the debug units sort last: neither is what somebody opening
    # the palette is looking for.
    def order(u):
        rank = 2 if u["group"] == "Other" else 1 if u["group"] == "Test and debug" else 0
        return (rank, u["group"], u["title"])

    out = sorted(units.values(), key=order)
    dest = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "src", "roster.json")
    with open(os.path.normpath(dest), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1, ensure_ascii=False)
        f.write("\n")
    commit = subprocess.run(["git", "-C", root, "rev-parse", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    grouped = len([u for u in out if u["group"] != "Other"])
    print("%d units, %d grouped, from Zero-K %s" % (len(out), grouped, commit))
    print("Pin this in game.rs: ROSTER_PIN = \"%s\"" % commit)

if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else sys.exit(__doc__))
