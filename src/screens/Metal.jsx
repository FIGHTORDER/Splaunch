import React from "react";
import { mapMetal } from "../net/splaunch.ts";

/* Where the metal is, drawn over the minimap.
 *
 * An author placing a metal extractor has to know where the metal is, and until
 * now the editor could not say - so a mex went down by eye, which is guessing.
 *
 * This draws the map's metal infomap: one byte a sample, sixteen elmos to a
 * sample, exactly as the engine reads it. It does not decide where "a spot" is.
 * That restraint is deliberate and `src-tauri/src/mapfile.rs` has the reason -
 * briefly, what counts as a spot is a choice rather than a fact, two reasonable
 * choices give two different answers from the same map, and Coilbox pins those
 * numbers in a shared catalogue precisely so that clients do not disagree.
 *
 * On a map whose metal sits in discrete blobs, which is most of them, the blobs
 * are the spots and this shows them. */

/** Fetch the infomap for `map`, or `null` while there is none. */
export function useMetalMap(map) {
  const [metal, setMetal] = React.useState(null);
  React.useEffect(() => {
    let live = true;
    setMetal(null);
    if (!map) return undefined;
    mapMetal(map).then(
      m => { if (live) setMetal(m); },
      // Not installed, or an archive this cannot open. The editor draws no
      // metal, which is what it did before.
      () => { if (live) setMetal(null); },
    );
    return () => { live = false; };
  }, [map]);
  return metal;
}

function decodeBase64(text) {
  const binary = atob(text);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

/**
 * The infomap as a translucent overlay, stretched to the board.
 *
 * Drawn at the infomap's own resolution and scaled by CSS, so the samples stay
 * square and a 384x512 grid does not have to become a 600px canvas to be seen.
 *
 * The floor is what keeps this readable: most of a map is zero or near it, and
 * painting every non-zero sample turns the whole board green. Only samples that
 * would actually be worth putting an extractor on are drawn.
 */
export default function MetalOverlay({ metal, opacity = 0.85 }) {
  const canvas = React.useRef(null);

  React.useEffect(() => {
    const el = canvas.current;
    if (!el || !metal) return;
    const ctx = el.getContext("2d");
    if (!ctx) return;
    const samples = decodeBase64(metal.samples);
    const image = ctx.createImageData(metal.width, metal.height);

    /* A tenth of full scale. Below that a sample is background noise on maps
       whose metal is smeared rather than placed, and drawing it hides the
       places that are actually worth building on. */
    const FLOOR = 25;
    for (let i = 0; i < samples.length; i += 1) {
      const v = samples[i];
      const at = i * 4;
      if (v < FLOOR) {
        image.data[at + 3] = 0;
        continue;
      }
      // Green through yellow to white as the sample gets richer, so a rich spot
      // reads differently from a poor one at a glance.
      const t = Math.min(1, (v - FLOOR) / (255 - FLOOR));
      image.data[at] = Math.round(80 + 175 * t);
      image.data[at + 1] = 255;
      image.data[at + 2] = Math.round(80 + 175 * t * t);
      image.data[at + 3] = Math.round(120 + 135 * t);
    }
    ctx.putImageData(image, 0, 0);
  }, [metal]);

  if (!metal) return null;
  return (
    <canvas
      ref={canvas}
      width={metal.width}
      height={metal.height}
      aria-hidden="true"
      style={{
        position: "absolute",
        inset: 0,
        width: "100%",
        height: "100%",
        opacity,
        pointerEvents: "none",
        // Nearest-neighbour: a sample is sixteen elmos of ground, and smoothing
        // it invents metal between the samples that are actually there.
        imageRendering: "pixelated",
        mixBlendMode: "screen",
      }}
    />
  );
}
