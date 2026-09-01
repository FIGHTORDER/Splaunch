import React from "react";
import { unitIcons, unitMarks } from "../net/splaunch.ts";

/* The game's own unit art, fetched as it is needed.
 *
 * Two kinds, because they are good at different sizes:
 *
 * - **Build pictures** are lit 3D renders. They are what a player recognises in
 *   a build menu, so the palette uses them.
 * - **Zoom-out icons** are flat silhouettes drawn to be read small, which is
 *   what a marker on the map is. They are shared - 275 units draw from 204
 *   types - and they carry no team colour of their own, so the marker tints
 *   them, the same way the game does.
 *
 * The archive holds hundreds of each, so both are asked for by name rather than
 * pulled across the bridge wholesale. A name that comes back with nothing is
 * remembered as nothing: absent means "not asked yet" and would loop. */

/** Shared machinery for both caches. */
function useArt(names, fetcher) {
  const [art, setArt] = React.useState({});
  // A ref, not state: it has to be updated before the fetch resolves, or the
  // next render starts the same request again.
  const asked = React.useRef(new Set());

  /* Keyed on the sorted, de-duplicated list. The effect must not depend on the
     value it writes - that is how a render loop starts. */
  const key = React.useMemo(
    () => [...new Set(names.filter(Boolean))].sort().join(" "),
    [names],
  );

  React.useEffect(() => {
    const wanted = key ? key.split(" ") : [];
    const missing = wanted.filter(n => !asked.current.has(n));
    if (!missing.length) return undefined;
    missing.forEach(n => asked.current.add(n));

    let live = true;
    const settle = got => {
      if (!live) return;
      setArt(prev => {
        const next = { ...prev };
        for (const n of missing) next[n] = got[n] ?? null;
        return next;
      });
    };
    // The editor works without art; it drew plain markers before.
    fetcher(missing).then(settle, () => settle({}));
    return () => { live = false; };
  }, [key, fetcher]);

  return art;
}

/** Build pictures, for the palette. */
export function useUnitIcons(names) {
  return useArt(names, unitIcons);
}

/** Zoom-out silhouettes, for the map. */
export function useUnitMarks(names) {
  return useArt(names, unitMarks);
}

function decodeBase64(text) {
  const binary = atob(text);
  const out = new Uint8ClampedArray(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

/**
 * One unit on the map, as its silhouette in its team's colour.
 *
 * Tinted rather than drawn as-is: the icons are the same for every side, so
 * without a tint a scenario is two identical armies. The source pixel's
 * brightness scales the tint, which keeps whatever shading the icon has instead
 * of flattening it to a solid blob.
 *
 * Falls back to a filled square when there is no icon - a machine with no
 * Zero-K installed is the state the editor opens in, and a unit still has to be
 * visible there.
 */
export function UnitMark({ mark, colour, selected, size = 22, neutral }) {
  const canvas = React.useRef(null);

  React.useEffect(() => {
    const el = canvas.current;
    if (!el || !mark) return undefined;
    const ctx = el.getContext("2d");
    if (!ctx) return undefined;
    const [tr, tg, tb] = colour;

    /* Tint in place: the icons are the same for every side, so untinted a
       scenario is two identical armies. The source pixel's brightness scales
       the tint, which keeps whatever shading the icon has rather than
       flattening it to a solid blob. */
    const tint = image => {
      const d = image.data;
      for (let i = 0; i < d.length; i += 4) {
        // Rec. 601 luma - what "how bright is this pixel" means here.
        const lum = (d[i] * 0.299 + d[i + 1] * 0.587 + d[i + 2] * 0.114) / 255;
        d[i] = tr * lum;
        d[i + 1] = tg * lum;
        d[i + 2] = tb * lum;
      }
      ctx.putImageData(image, 0, 0);
    };

    // Straight from a .dds: the pixels are already here.
    if (mark.pixels) {
      const px = decodeBase64(mark.pixels);
      const image = ctx.createImageData(mark.width, mark.height);
      image.data.set(px);
      tint(image);
      return undefined;
    }

    /* Already drawable, so the browser decodes it and we read the pixels back
       to tint them. Cancelled on unmount because it lands a frame later. */
    if (!mark.src) return undefined;
    let live = true;
    const img = new Image();
    img.onload = () => {
      if (!live) return;
      ctx.clearRect(0, 0, mark.width, mark.height);
      ctx.drawImage(img, 0, 0, mark.width, mark.height);
      tint(ctx.getImageData(0, 0, mark.width, mark.height));
    };
    img.src = mark.src;
    return () => { live = false; };
  }, [mark, colour]);

  const side = selected ? size + 4 : size;
  const css = `rgb(${colour[0]},${colour[1]},${colour[2]})`;
  const ring = selected
    ? "0 0 0 1px #000, 0 0 0 3px #fff"
    : "0 0 0 1px rgba(0,0,0,.7)";

  if (!mark) {
    return (
      <span style={{ display: "block", width: side, height: side, boxShadow: ring,
        borderRadius: neutral ? "50%" : 2, background: css }} />
    );
  }

  return (
    <canvas
      ref={canvas}
      width={mark.width}
      height={mark.height}
      style={{
        display: "block",
        width: side,
        height: side,
        // The silhouette is the shape, so the ring goes round the art rather
        // than round a box drawn behind it.
        filter: `drop-shadow(0 0 1px rgba(0,0,0,.9))${selected ? " drop-shadow(0 0 2px #fff)" : ""}`,
      }}
    />
  );
}
