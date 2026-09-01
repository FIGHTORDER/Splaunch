import React from "react";

/* A minimap drawn in the map's own proportions.
 *
 * This is not Shiro's MapImage, for two reasons that compound.
 *
 * 1. MapImage hardcodes `object-fit: cover`, which crops. On a map that is not
 *    square - 145 of the catalogue's 343 - that throws away the ends of the
 *    map, and an author cannot place a unit somewhere they cannot see.
 *
 * 2. zero-k.info's `.minimap.jpg` is not in the map's proportions. Its aspect
 *    ratio is the *square* of the real one. Measured against a map that states
 *    its size in its own name: Tuckedup_16x12_003 is 16x12, the catalogue
 *    agrees, and the image is 1024x575 - which is (16/12)^2, not 16/12. Same
 *    relation on DesertSiege_v2b (20x12 -> 1024x368), Icy Run v2 (12x4 ->
 *    1024x113), Chicken_Farm_v02 (6x16 -> 144x1024) and Comet Catcher Redux
 *    (12x16 -> 576x1024). The `.thumbnail.jpg` is correctly proportioned but is
 *    96px on its long side, so it is no use as a board.
 *
 * The whole map is in that image, squashed - not cropped. Resizing it
 * non-uniformly onto the correctly proportioned thumbnail matches at a mean
 * absolute error of 11.8, against 28.3 for the best centre crop. So the fix is
 * `object-fit: fill` into a box of the true proportions: that undoes the squash
 * exactly and keeps all 1024 pixels of it.
 *
 * MapImage is vendored from Shiro and is not this repo's to maintain, so this
 * lives here rather than as a patch to it. Shiro has the same bug.
 */

const BASE = "https://zero-k.info/Resources/";

/** The map's width over its depth, or 1 when the catalogue does not say. */
export function mapAspect(width, height) {
  return width && height ? width / height : 1;
}

/**
 * Sizing for a minimap letterboxed inside a fixed slot, at its true shape.
 *
 * One axis is pinned and the other left to `aspect-ratio`, because pinning both
 * and clamping with max-* lets the browser satisfy the clamp by breaking the
 * ratio - which is the thing this whole file exists to avoid.
 */
export function containBox(aspect) {
  return {
    aspectRatio: String(aspect),
    width: aspect >= 1 ? "100%" : "auto",
    height: aspect >= 1 ? "auto" : "100%",
    maxWidth: "100%",
    maxHeight: "100%",
  };
}

export default function Minimap({ map, saturate = 0.9, style, children }) {
  const [failed, setFailed] = React.useState(false);
  const [loaded, setLoaded] = React.useState(false);
  React.useEffect(() => { setFailed(false); setLoaded(false); }, [map]);

  /* Assets are stored with underscores and the catalogue sends spaces, so an
     unnormalised name 404s for most maps. Kept from MapImage, where it is a
     vendor patch. */
  const src = `${BASE}${encodeURIComponent(String(map).replace(/ /g, "_"))}.minimap.jpg`;

  return (
    <div style={{ position: "relative", overflow: "hidden",
      background: "var(--ink-000)", ...style }}>
      {failed ? (
        <div style={{ position: "absolute", inset: 0, display: "flex",
          alignItems: "center", justifyContent: "center", textAlign: "center",
          padding: "var(--sp-4)", background: "var(--surface-sunken)" }}>
          <span style={{ font: "var(--text-label)", letterSpacing: "var(--track-label)",
            textTransform: "uppercase", color: "var(--text-low)", wordBreak: "break-word" }}>
            {map}
          </span>
        </div>
      ) : (
        /* Lazily, because the picker draws up to 48 of these at once and every
           one is a request to zero-k.info. Coilbox's author reports that this
           shape of traffic against the game infrastructure had real
           consequences at BAR, including a dead endpoint; the browser only
           fetching what is scrolled into view costs one attribute. */
        <img src={src} alt="" loading="lazy" decoding="async"
          onError={() => setFailed(true)} onLoad={() => setLoaded(true)}
          style={{ width: "100%", height: "100%", objectFit: "fill", display: "block",
            filter: `saturate(${saturate})`, opacity: loaded ? 1 : 0,
            transition: "opacity var(--dur-base) var(--ease-out)" }} />
      )}
      {children}
    </div>
  );
}

/** The name over the bottom of a card, with the version trailing off. */
export function MinimapCaption({ map, note }) {
  const m = String(map).match(/^(.*?)((?:_v?\d[\d.]*)?)$/) || [];
  return (
    <>
      <div style={{ position: "absolute", inset: 0, background: "var(--protect-bottom)",
        pointerEvents: "none" }} />
      <div style={{ position: "absolute", left: "var(--sp-4)", right: "var(--sp-4)",
        bottom: "var(--sp-3)", font: "var(--w-semibold) var(--size-tiny)/1.2 var(--font-core)",
        color: "var(--white)", whiteSpace: "nowrap", overflow: "hidden",
        textOverflow: "ellipsis", pointerEvents: "none" }}>
        {m[1]}<span style={{ color: "var(--fff-56)" }}>{m[2]}</span>
        {note && <span style={{ color: "var(--fff-56)" }}> · {note}</span>}
      </div>
    </>
  );
}
