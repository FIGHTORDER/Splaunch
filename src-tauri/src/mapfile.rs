//! Where the metal is, read out of the map's own file.
//!
//! The catalogue carries a map's name, its id and its size, and nothing about
//! its ground. An author placing a metal extractor has to know where the metal
//! is, and until now the editor could not say - so a mex went down by eye
//! against a minimap, which is guessing.
//!
//! ## Density, not spots
//!
//! This returns the map's metal infomap: one byte a sample, sixteen elmos to a
//! sample, exactly as the engine reads it. It does **not** decide where "a
//! spot" is, and that restraint is deliberate.
//!
//! Coilbox's notes on the same problem are worth repeating, because it is the
//! author of the thing Splaunch is trying to stay compatible with saying it:
//! every other fact about a map is a value read out of the archive, so two
//! clients cannot disagree about it, but what counts as a spot is a *choice* -
//! peak height, separation, floor - and two reasonable choices give two
//! different answers from identical input. Coilbox pins those numbers in a
//! shared catalogue with its own version so that clients agree. Inventing a
//! second definition here would produce exactly the divergence that file exists
//! to prevent, and the symptom would be two editors reporting different facts
//! about the same map.
//!
//! Drawing the density says everything an author needs - on a map whose metal
//! is in discrete blobs, which is most of them, the blobs *are* the spots - and
//! claims nothing this repository is entitled to claim. If the shared catalogue
//! becomes reachable, discrete spots can be drawn on top of this with its
//! numbers rather than ours.
//!
//! ## The layout
//!
//! From the engine's `SMFFormat.h`: sixteen bytes of magic, then `version`,
//! `mapid`, `mapx`, `mapy`, `squareSize`, `texelPerSquare`, `tilesize` as
//! little-endian `int`s, `minHeight` and `maxHeight` as `float`s, then
//! `heightmapPtr`, `typeMapPtr`, `tilesPtr`, `minimapPtr`, `metalmapPtr` and
//! `featurePtr`. That puts `featurePtr` at 72, which is the offset the vendored
//! Coilbox reader uses and has run against real maps - so the arithmetic that
//! puts `metalmapPtr` at 68 is anchored to something known to work rather than
//! to memory.
//!
//! `mapx` and `mapy` are in map squares. The metal infomap is half that in each
//! direction, one byte a sample.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

const MAGIC: &[u8] = b"spring map file";
const OFF_MAPX: usize = 24;
const OFF_MAPY: usize = 28;
const OFF_METALMAP_PTR: usize = 68;

/// Largest `.smf` this will read, matching the vendored Coilbox reader's cap.
/// What stops a malformed header turning into an allocation.
const MAX_SMF_BYTES: usize = 64 * 1024 * 1024;

/// A map's metal infomap.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetalMap {
    pub width: u32,
    pub height: u32,
    /// One byte a sample, row-major, base64 in the standard alphabet.
    ///
    /// Not a JSON array: a 12x16 map is 384x512, and two hundred thousand
    /// numbers spelled out in JSON is several megabytes to say what a quarter
    /// of a megabyte of base64 says.
    pub samples: String,
}

fn le_i32(bytes: &[u8], at: usize) -> Option<i32> {
    let slice = bytes.get(at..at + 4)?;
    Some(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

/// The metal infomap inside an `.smf`.
pub fn metal_map(smf: &[u8]) -> Option<MetalMap> {
    if !smf.starts_with(MAGIC) {
        return None;
    }
    let mapx = le_i32(smf, OFF_MAPX)?;
    let mapy = le_i32(smf, OFF_MAPY)?;
    let ptr = le_i32(smf, OFF_METALMAP_PTR)?;
    if mapx <= 0 || mapy <= 0 || ptr <= 0 {
        return None;
    }
    // Half the map squares in each direction, one byte a sample.
    let width = u32::try_from(mapx).ok()? / 2;
    let height = u32::try_from(mapy).ok()? / 2;
    let count = (width as usize).checked_mul(height as usize)?;
    if width == 0 || height == 0 || count > MAX_SMF_BYTES {
        return None;
    }
    let start = usize::try_from(ptr).ok()?;
    let samples = smf.get(start..start.checked_add(count)?)?;
    Some(MetalMap {
        width,
        height,
        samples: crate::game::base64_standard(samples),
    })
}

/// The `.smf` inside a map archive.
///
/// Both container formats, because Zero-K's maps are `.sd7` - 7-zip - and a few
/// are `.sdz`, which is a zip. The rest of this crate only ever needed zip,
/// which is why maps could not be read until now.
pub fn smf_bytes(archive: &Path) -> Option<Vec<u8>> {
    let is = |ext: &str| {
        matches!(archive.extension().and_then(|e| e.to_str()), Some(e) if e.eq_ignore_ascii_case(ext))
    };
    if is("sd7") {
        return smf_from_7z(archive);
    }
    if is("sdz") {
        return smf_from_zip(archive);
    }
    if is("smf") {
        return std::fs::read(archive).ok().filter(|b| b.starts_with(MAGIC));
    }
    None
}

/// Whether an entry inside a map archive is the map itself.
///
/// A map archive holds one `.smf` and the `.smt` of tiles beside it; anything
/// else is configuration. Matched by extension rather than by a path, because
/// archives disagree about whether it sits in `maps/`.
fn is_smf(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".smf")
}

fn smf_from_zip(archive: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(archive).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).ok()?;
    let index = (0..zip.len())
        .find(|i| zip.by_index_raw(*i).map(|e| is_smf(e.name())).unwrap_or(false))?;
    let mut entry = zip.by_index(index).ok()?;
    if entry.size() as usize > MAX_SMF_BYTES {
        return None;
    }
    let mut out = Vec::new();
    entry.read_to_end(&mut out).ok()?;
    Some(out)
}

fn smf_from_7z(archive: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(archive).ok()?;
    let len = file.metadata().ok()?.len();
    let mut reader =
        sevenz_rust2::SevenZReader::new(std::io::BufReader::new(file), len, Default::default())
            .ok()?;
    let mut found: Option<Vec<u8>> = None;
    reader
        .for_each_entries(|entry, body| {
            if found.is_some() || !is_smf(entry.name()) {
                return Ok(true);
            }
            if entry.size() as usize > MAX_SMF_BYTES {
                return Ok(true);
            }
            let mut out = Vec::new();
            body.read_to_end(&mut out)?;
            found = Some(out);
            // Stop: the tiles beside it are far larger and nothing here wants them.
            Ok(false)
        })
        .ok()?;
    found
}

/// The archive holding `map`, by the name a scenario carries.
///
/// Uses the same normalisation `game::resolve_map` does, because a scenario
/// carries whatever its author typed and the file is
/// `comet_catcher_redux.sd7`.
pub fn map_archive(root: &Path, map: &str) -> Option<PathBuf> {
    let normal = |s: &str| s.to_ascii_lowercase().replace([' ', '_', '-'], "");
    let wanted = normal(map);
    if wanted.is_empty() {
        return None;
    }
    let entries = std::fs::read_dir(root.join("maps")).ok()?;
    let mut prefix: Option<PathBuf> = None;
    let mut prefix_count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let ok = matches!(path.extension().and_then(|e| e.to_str()), Some(e)
            if e.eq_ignore_ascii_case("sd7")
                || e.eq_ignore_ascii_case("sdz")
                || e.eq_ignore_ascii_case("smf"));
        if !ok {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let stem = normal(stem);
        if stem == wanted {
            return Some(path);
        }
        // A scenario often names the map without the version its file carries.
        if stem.starts_with(&wanted) {
            prefix_count += 1;
            prefix = Some(path);
        }
    }
    // Two versions side by side is a real situation, and opening the wrong one
    // silently is worse than drawing no metal at all.
    if prefix_count == 1 { prefix } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An `.smf` with a known metal map in it.
    fn smf(mapx: i32, mapy: i32, samples: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; 80];
        out[..MAGIC.len()].copy_from_slice(MAGIC);
        out[OFF_MAPX..OFF_MAPX + 4].copy_from_slice(&mapx.to_le_bytes());
        out[OFF_MAPY..OFF_MAPY + 4].copy_from_slice(&mapy.to_le_bytes());
        out[OFF_METALMAP_PTR..OFF_METALMAP_PTR + 4].copy_from_slice(&80i32.to_le_bytes());
        out.extend_from_slice(samples);
        out
    }

    #[test]
    fn the_metal_map_is_half_the_map_squares_each_way() {
        // A 4x2 square map carries a 2x1 infomap, which is the engine's rule.
        let map = metal_map(&smf(4, 2, &[10, 200])).expect("no metal map");
        assert_eq!((map.width, map.height), (2, 1));
        assert_eq!(map.samples, crate::game::base64_standard(&[10, 200]));
    }

    #[test]
    fn a_file_that_is_not_a_map_is_refused() {
        assert!(metal_map(b"not a map at all, really not").is_none());
        // Right magic, nonsense header.
        assert!(metal_map(&smf(0, 0, &[])).is_none());
        assert!(metal_map(&smf(-4, 4, &[1])).is_none());
    }

    #[test]
    fn a_pointer_past_the_end_is_refused_rather_than_panicking() {
        /* The header is bytes off a disk somebody else wrote. A truncated map
           must come back as "no metal" rather than take the editor down. */
        let mut bytes = smf(4, 2, &[1, 2]);
        bytes.truncate(81); // one sample short of what the header promises
        assert!(metal_map(&bytes).is_none());

        let mut wild = smf(4, 2, &[1, 2]);
        wild[OFF_METALMAP_PTR..OFF_METALMAP_PTR + 4]
            .copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(metal_map(&wild).is_none());
    }

    #[test]
    fn an_absurd_size_does_not_become_an_allocation() {
        let mut huge = smf(4, 2, &[1, 2]);
        huge[OFF_MAPX..OFF_MAPX + 4].copy_from_slice(&i32::MAX.to_le_bytes());
        huge[OFF_MAPY..OFF_MAPY + 4].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(metal_map(&huge).is_none());
    }

    #[test]
    fn the_map_is_found_inside_a_zip_archive() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("splaunch-mapfile-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tinymap.sdz");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("mapinfo.lua", opts).unwrap();
        zip.write_all(b"return {}").unwrap();
        zip.start_file("maps/tiny.smf", opts).unwrap();
        zip.write_all(&smf(4, 2, &[7, 9])).unwrap();
        zip.finish().unwrap();

        let bytes = smf_bytes(&path).expect("no smf found in the archive");
        let map = metal_map(&bytes).expect("no metal map");
        assert_eq!((map.width, map.height), (2, 1));
        assert_eq!(map.samples, crate::game::base64_standard(&[7, 9]));
    }

    #[test]
    fn a_name_without_its_version_still_finds_the_file() {
        let dir = std::env::temp_dir().join("splaunch-mapfind-test").join("maps");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("comet_catcher_redux_v3.sd7"), b"x").unwrap();
        let root = dir.parent().unwrap();

        assert!(map_archive(root, "Comet Catcher Redux").is_some());
        assert!(map_archive(root, "Nothing Like It").is_none());

        // Two versions side by side: neither, rather than the wrong one.
        std::fs::write(dir.join("comet_catcher_redux_v4.sd7"), b"x").unwrap();
        assert!(map_archive(root, "Comet Catcher Redux").is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
