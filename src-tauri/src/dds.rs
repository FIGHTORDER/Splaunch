//! Just enough DDS to draw Zero-K's zoom-out unit icons.
//!
//! Those icons are the flat silhouettes the game shows when you zoom out, and
//! they are what a top-down editor wants: a build picture is a lit 3D render
//! that turns to mush at twenty pixels, while these were drawn to be read at
//! exactly that size.
//!
//! They are DDS, which no browser will display. Measured over the roster's 204
//! distinct icon types: 169 are DXT3, 8 DXT1, 4 DXT5 and 3 uncompressed 32-bit,
//! every one of them 64x64 or 128x128. Only 65 of the units have an icon that
//! also ships as a PNG, so "prefer the PNG" covers a quarter of the roster and
//! leaves the rest as blocks.
//!
//! So this decodes them. It is about a hundred lines because the formats are
//! small and old, and it adds no dependency: the result goes to the frontend as
//! raw RGBA for a canvas rather than as an encoded image, which means no PNG
//! writer either.
//!
//! Only what those files actually are is implemented. Anything else returns
//! `None` and the caller draws the plain marker, because a wrong guess at a
//! pixel format is a smear rather than an error.

use serde::Serialize;

/// A decoded image, ready for `putImageData`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, base64 in the standard alphabet.
    pub pixels: String,
}

const HEADER: usize = 128;

fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn rgb565(v: u16) -> [u8; 3] {
    // Replicate the high bits into the low ones so full-scale stays full-scale:
    // 0x1F must become 0xFF, not 0xF8.
    let r = ((v >> 11) & 0x1F) as u8;
    let g = ((v >> 5) & 0x3F) as u8;
    let b = (v & 0x1F) as u8;
    [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]
}

/// The four colours a DXT block interpolates between.
///
/// `punchthrough` is DXT1's one-bit alpha mode, where `c0 <= c1` means the
/// fourth entry is transparent. DXT3 and DXT5 carry alpha separately and always
/// use the four-colour form, so they pass `false` - reading their colour block
/// the DXT1 way turns a legitimately dark texel transparent.
fn palette(block: &[u8], punchthrough: bool) -> [[u8; 4]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let a = rgb565(c0);
    let b = rgb565(c1);
    let mut out = [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [0; 4], [0; 4]];
    let mix = |x: u8, y: u8, wx: u16, wy: u16, d: u16| {
        (((x as u16) * wx + (y as u16) * wy) / d) as u8
    };
    if !punchthrough || c0 > c1 {
        for i in 0..3 {
            out[2][i] = mix(a[i], b[i], 2, 1, 3);
            out[3][i] = mix(a[i], b[i], 1, 2, 3);
        }
        out[2][3] = 255;
        out[3][3] = 255;
    } else {
        for i in 0..3 {
            out[2][i] = mix(a[i], b[i], 1, 1, 2);
        }
        out[2][3] = 255;
        out[3] = [0, 0, 0, 0];
    }
    out
}

/// Write one 4x4 colour block into the image.
fn put_block(
    out: &mut [u8],
    dims: (u32, u32),
    origin: (u32, u32),
    colours: &[[u8; 4]; 4],
    indices: u32,
    alpha: &[u8; 16],
) {
    let (width, height) = dims;
    for y in 0..4u32 {
        for x in 0..4u32 {
            let (px, py) = (origin.0 + x, origin.1 + y);
            if px >= width || py >= height {
                continue;
            }
            let texel = (y * 4 + x) as usize;
            let mut rgba = colours[((indices >> (2 * texel)) & 3) as usize];
            // The block's own alpha wins where the format carries one; DXT1
            // passes 255s through so its punch-through survives.
            rgba[3] = ((rgba[3] as u16 * alpha[texel] as u16) / 255) as u8;
            let at = ((py * width + px) * 4) as usize;
            if let Some(slot) = out.get_mut(at..at + 4) {
                slot.copy_from_slice(&rgba);
            }
        }
    }
}

/// DXT5's eight-byte interpolated alpha block, as sixteen texels.
fn dxt5_alpha(block: &[u8]) -> [u8; 16] {
    let (a0, a1) = (block[0], block[1]);
    let mut lut = [0u8; 8];
    lut[0] = a0;
    lut[1] = a1;
    if a0 > a1 {
        for i in 0..6 {
            lut[2 + i] = (((6 - i) as u16 * a0 as u16 + (1 + i) as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 0..4 {
            lut[2 + i] = (((4 - i) as u16 * a0 as u16 + (1 + i) as u16 * a1 as u16) / 5) as u8;
        }
        lut[6] = 0;
        lut[7] = 255;
    }
    // Six bytes of three-bit indices, little-endian across the whole run.
    let bits = (block[2] as u64)
        | (block[3] as u64) << 8
        | (block[4] as u64) << 16
        | (block[5] as u64) << 24
        | (block[6] as u64) << 32
        | (block[7] as u64) << 40;
    let mut out = [0u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = lut[((bits >> (3 * i)) & 7) as usize];
    }
    out
}

/// Decode a DDS into RGBA, or `None` if it is not one this understands.
pub fn decode(bytes: &[u8]) -> Option<Rgba> {
    if !bytes.starts_with(b"DDS ") || bytes.len() < HEADER {
        return None;
    }
    let height = le_u32(bytes, 12)?;
    let width = le_u32(bytes, 16)?;
    // These are unit icons. The cap is what stops a bad header allocating.
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return None;
    }
    let fourcc = bytes.get(84..88)?;
    let rgb_bits = le_u32(bytes, 88)?;
    let body = bytes.get(HEADER..)?;
    let mut out = vec![0u8; (width as usize) * (height as usize) * 4];

    let block_bytes = match fourcc {
        b"DXT1" => 8,
        b"DXT3" | b"DXT5" => 16,
        _ => {
            // Uncompressed 32-bit. Spring writes these as BGRA.
            if rgb_bits != 32 {
                return None;
            }
            let need = out.len();
            let src = body.get(..need)?;
            for (dst, px) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                dst.copy_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            return Some(Rgba {
                width,
                height,
                pixels: crate::game::base64_standard(&out),
            });
        }
    };

    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let opaque = [255u8; 16];
    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let at = ((by * blocks_x + bx) as usize) * block_bytes;
            let block = body.get(at..at + block_bytes)?;
            let (colour, alpha) = match fourcc {
                b"DXT1" => (block, opaque),
                b"DXT3" => {
                    // Eight bytes of four-bit alpha, low nibble first.
                    let mut a = [0u8; 16];
                    for (i, slot) in a.iter_mut().enumerate() {
                        let nibble = if i % 2 == 0 {
                            block[i / 2] & 0x0F
                        } else {
                            block[i / 2] >> 4
                        };
                        *slot = nibble * 17;
                    }
                    (&block[8..], a)
                }
                _ => (&block[8..], dxt5_alpha(block)),
            };
            let colours = palette(colour, fourcc == b"DXT1");
            let indices = le_u32(colour, 4)?;
            put_block(&mut out, (width, height), (bx * 4, by * 4), &colours, indices, &alpha);
        }
    }
    Some(Rgba {
        width,
        height,
        pixels: crate::game::base64_standard(&out),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(w: u32, h: u32, fourcc: &[u8; 4], bits: u32) -> Vec<u8> {
        let mut b = vec![0u8; HEADER];
        b[..4].copy_from_slice(b"DDS ");
        b[12..16].copy_from_slice(&h.to_le_bytes());
        b[16..20].copy_from_slice(&w.to_le_bytes());
        b[84..88].copy_from_slice(fourcc);
        b[88..92].copy_from_slice(&bits.to_le_bytes());
        b
    }

    fn pixels(img: &Rgba) -> Vec<u8> {
        // Round-trips our own base64, so the test reads pixels rather than text.
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut acc = 0u32;
        let mut bits = 0;
        for c in img.pixels.bytes().filter(|c| *c != b'=') {
            let v = alphabet.iter().position(|a| *a == c).unwrap() as u32;
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
            }
        }
        out
    }

    #[test]
    fn a_solid_dxt1_block_decodes_to_its_colour() {
        // c0 = pure red in 565, c1 = 0, every index 0.
        let mut dds = header(4, 4, b"DXT1", 0);
        dds.extend_from_slice(&0xF800u16.to_le_bytes());
        dds.extend_from_slice(&0u16.to_le_bytes());
        dds.extend_from_slice(&0u32.to_le_bytes());
        let img = decode(&dds).expect("did not decode");
        assert_eq!((img.width, img.height), (4, 4));
        // 0x1F must scale to 0xFF, not 0xF8 - the whole point of replicating
        // the high bits down.
        assert_eq!(&pixels(&img)[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn dxt3_alpha_is_four_bits_a_texel_low_nibble_first() {
        let mut dds = header(4, 4, b"DXT3", 0);
        // First texel alpha 0, second 0xF, rest 0.
        let mut alpha = [0u8; 8];
        alpha[0] = 0xF0;
        dds.extend_from_slice(&alpha);
        dds.extend_from_slice(&0xF800u16.to_le_bytes());
        dds.extend_from_slice(&0u16.to_le_bytes());
        dds.extend_from_slice(&0u32.to_le_bytes());
        let px = pixels(&decode(&dds).unwrap());
        assert_eq!(px[3], 0, "first texel should be transparent");
        assert_eq!(px[7], 255, "second texel should be opaque");
    }

    #[test]
    fn dxt3_does_not_use_dxt1s_punchthrough_rule() {
        /* With c0 <= c1 the DXT1 reading makes index 3 transparent. DXT3 keeps
           its alpha in its own block and always has four opaque colours, so
           reading it the DXT1 way silently holes any texel using that index. */
        let mut dds = header(4, 4, b"DXT3", 0);
        dds.extend_from_slice(&[0xFF; 8]); // fully opaque
        dds.extend_from_slice(&0x0001u16.to_le_bytes()); // c0 < c1
        dds.extend_from_slice(&0xF800u16.to_le_bytes());
        dds.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // every texel index 3
        let px = pixels(&decode(&dds).unwrap());
        assert_eq!(px[3], 255, "a DXT3 texel was made transparent by the DXT1 rule");
    }

    #[test]
    fn an_uncompressed_dds_is_read_as_bgra() {
        let mut dds = header(1, 1, &[0, 0, 0, 0], 32);
        dds.extend_from_slice(&[10, 20, 30, 40]); // B G R A
        assert_eq!(pixels(&decode(&dds).unwrap()), vec![30, 20, 10, 40]);
    }

    /// Decode real icons out of a Zero-K checkout or install.
    ///
    /// The tests above prove this reads the format the way I understand it,
    /// which is not the same as reading the files Zero-K actually ships - that
    /// distinction is what put an unplaceable `facing` into a released
    /// scenario. Point this at real ones:
    ///
    /// ```text
    /// SPLAUNCH_TEST_ICON_DIR=/path/to/Zero-K/icons \
    ///   cargo test --lib -- --ignored --nocapture dds
    /// ```
    #[test]
    #[ignore = "needs real icons in SPLAUNCH_TEST_ICON_DIR"]
    fn real_zero_k_icons_decode() {
        let dir = std::env::var("SPLAUNCH_TEST_ICON_DIR")
            .expect("set SPLAUNCH_TEST_ICON_DIR to a Zero-K icons directory");
        let mut seen = 0;
        let mut failed = Vec::new();
        let mut blank = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("cannot read that directory").flatten() {
            let path = entry.path();
            if !matches!(path.extension().and_then(|e| e.to_str()), Some(e) if e.eq_ignore_ascii_case("dds")) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            seen += 1;
            match decode(&bytes) {
                None => failed.push(path.file_name().unwrap().to_string_lossy().into_owned()),
                Some(img) => {
                    let px = pixels(&img);
                    // An icon is a silhouette: some of it has to be opaque and
                    // some transparent, or the decode produced a flat sheet.
                    let opaque = px.chunks_exact(4).filter(|p| p[3] > 200).count();
                    let clear = px.chunks_exact(4).filter(|p| p[3] < 40).count();
                    if opaque == 0 || clear == 0 {
                        blank.push(format!(
                            "{} ({}x{}, {opaque} opaque, {clear} clear)",
                            path.file_name().unwrap().to_string_lossy(),
                            img.width,
                            img.height
                        ));
                    }
                }
            }
        }
        println!("decoded {} of {seen} icons", seen - failed.len());
        assert!(seen > 0, "no .dds files in {dir}");
        assert!(failed.is_empty(), "failed to decode: {failed:?}");
        println!("flat (no silhouette): {}", blank.len());
        for b in blank.iter().take(10) {
            println!("  {b}");
        }
    }

    #[test]
    fn anything_else_is_refused_rather_than_smeared() {
        assert!(decode(b"not a dds").is_none());
        // A format this does not implement: better a plain marker than a smear.
        let mut dxt2 = header(4, 4, b"DXT2", 0);
        dxt2.extend_from_slice(&[0; 16]);
        assert!(decode(&dxt2).is_none());
        // A header promising more than the file holds.
        let truncated = header(64, 64, b"DXT3", 0);
        assert!(decode(&truncated).is_none());
    }
}
