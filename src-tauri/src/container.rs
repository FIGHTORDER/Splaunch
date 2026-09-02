//! Coilbox's container envelope, which is the contract for anything shared.
//!
//! Ported from `src/container/container.ts` in <https://github.com/tomjn/coilbox>
//! at commit `20cbdf4d64e6`, MIT. The shape is theirs and is not ours to
//! redesign: the point of implementing it is that a file written by one tool
//! opens in the other, and a second dialect would defeat that.
//!
//! ```text
//! { format: "coilbox", container: 1, kind: "preset", kindVersion: 1, payload: {...} }
//! ```
//!
//! Two independent version numbers rather than semver, deliberately. `container`
//! versions the envelope and `kindVersion` the payload, so a reader can tell
//! "this is a preset I am too old to read" from "this is not a container at
//! all" and say so, instead of half-reading it. [`identify`] answers that
//! without looking inside the payload, which is what lets a refusal be polite.
//!
//! Fields nobody here knows about are carried through untouched rather than
//! dropped, because upstream adds fields *without* bumping `kindVersion` on the
//! understanding that older readers ignore them. Dropping them on a round trip
//! would quietly destroy data that a newer Coilbox put there.

use serde::{Deserialize, Serialize};

/// The only value `format` ever takes. Present so a stray JSON file is refused
/// by name rather than by a confusing field-level error.
pub const CONTAINER_FORMAT: &str = "coilbox";

/// The envelope version this build writes and reads.
pub const CONTAINER_VERSION: u32 = 1;

/// Prefix on the compressed, pasteable form: DEFLATE, then base64url.
pub const CODE_PREFIX: &str = "cbz1.";

/// One shared artefact, envelope and all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Container<P> {
    pub format: String,
    pub container: u32,
    pub kind: String,
    pub kind_version: u32,
    pub payload: P,
}

impl<P> Container<P> {
    /// Wrap a payload for sharing.
    pub fn new(kind: &str, kind_version: u32, payload: P) -> Self {
        Container {
            format: CONTAINER_FORMAT.to_string(),
            container: CONTAINER_VERSION,
            kind: kind.to_string(),
            kind_version,
            payload,
        }
    }
}

/// Whether this build can read a container, and if not, why not.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Compatibility {
    /// Readable.
    Supported,
    /// Written by something newer. Refuse, do not guess.
    Newer,
    /// A container, but of a kind this build has no reader for.
    UnknownKind,
}

/// What a container says it is, without validating what is inside it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub kind: String,
    pub kind_version: u32,
    pub container: u32,
    pub compatibility: Compatibility,
}

/// The kinds this build reads, and the highest payload version of each.
///
/// Upstream defines seven kinds; the rest are listed in `vendor/coilbox/INTEROP.md`
/// and are deliberately absent here rather than half-supported.
const SUPPORTED: &[(&str, u32)] = &[("preset", 1)];

/// Read the envelope and say whether it can be opened.
///
/// Deliberately tolerant of the payload: a preset carrying a field this build
/// has never heard of is still a preset, and refusing it here would make every
/// upstream addition a breaking change.
pub fn identify(text: &str) -> Result<Identity, String> {
    let value = decode_text(text)?;
    let object = value.as_object().ok_or("that file is not a container")?;

    let format = object.get("format").and_then(|v| v.as_str()).unwrap_or("");
    if format != CONTAINER_FORMAT {
        return Err(format!(
            "that is not a Coilbox file - its format says {:?} rather than {CONTAINER_FORMAT:?}",
            format
        ));
    }
    let container = object.get("container").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let kind = object
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let kind_version = object
        .get("kindVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    let compatibility = if container > CONTAINER_VERSION {
        Compatibility::Newer
    } else {
        match SUPPORTED.iter().find(|(k, _)| *k == kind) {
            None => Compatibility::UnknownKind,
            Some((_, highest)) if kind_version > *highest => Compatibility::Newer,
            Some(_) => Compatibility::Supported,
        }
    };
    Ok(Identity { kind, kind_version, container, compatibility })
}

/// A container's text in any of the three forms upstream accepts.
///
/// Raw JSON is what "Share to file" writes. The `cbz1.` form is the pasteable
/// code. The third is a bare base64url payload with no prefix, which upstream
/// still reads because it wrote some before the prefix existed.
pub fn decode_text(text: &str) -> Result<serde_json::Value, String> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).map_err(|e| format!("that is not valid JSON: {e}"));
    }
    let body = trimmed.strip_prefix(CODE_PREFIX);
    let compressed = body.is_some();
    let raw = base64url_decode(body.unwrap_or(trimmed))
        .ok_or("that is neither JSON nor a Coilbox share code")?;
    let bytes = if compressed { inflate(&raw)? } else { raw };
    serde_json::from_slice(&bytes).map_err(|e| format!("that share code does not hold JSON: {e}"))
}

/// Read a container of a known kind, checking the envelope first.
pub fn open<P: serde::de::DeserializeOwned>(text: &str, want: &str) -> Result<Container<P>, String> {
    let id = identify(text)?;
    match id.compatibility {
        Compatibility::Newer => {
            return Err(format!(
                "this {} was written by a newer Coilbox (container {}, payload {}). \
                 Splaunch reads container {CONTAINER_VERSION}.",
                id.kind, id.container, id.kind_version
            ))
        }
        Compatibility::UnknownKind => {
            return Err(format!(
                "Splaunch does not read Coilbox {:?} files yet - only {}.",
                id.kind,
                SUPPORTED.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", ")
            ))
        }
        Compatibility::Supported => {}
    }
    if id.kind != want {
        return Err(format!("that is a {:?} file, not a {want}.", id.kind));
    }
    let value = decode_text(text)?;
    serde_json::from_value(value).map_err(|e| format!("that {want} is malformed: {e}"))
}

/// The JSON a "Share to file" writes: pretty, so a human can read a diff of it.
pub fn to_json<P: Serialize>(container: &Container<P>) -> Result<String, String> {
    serde_json::to_string_pretty(container).map_err(|e| format!("could not write it out: {e}"))
}

// ---------------------------------------------------------------- codecs ---

/// base64url, padding optional. Not `customkey`'s: that one is a port of
/// Zero-K's *broken* decoder and exists to prove our payloads survive it.
fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a') as u32 + 26,
            b'0'..=b'9' => (c - b'0') as u32 + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        })
    };
    let chars: Vec<u8> = text
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
        .collect();
    if chars.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(chars.len() * 3 / 4);
    for chunk in chars.chunks(4) {
        let mut bits = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            bits |= value(*c)? << (18 - 6 * i);
        }
        // A 4-char chunk carries 3 bytes, a 3-char one 2, a 2-char one 1.
        for i in 0..chunk.len().saturating_sub(1) {
            out.push((bits >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// The most a share code may become once decompressed.
///
/// A preset is a few kilobytes of JSON. This is a thousand times that and still
/// far below what a pasted string can be made to expand to: DEFLATE will turn a
/// few megabytes of the right input into gigabytes, and this runs on whatever
/// somebody pasted into the box.
const MAX_INFLATED: u64 = 8 * 1024 * 1024;

/// Raw DEFLATE, which is what upstream compresses a share code with.
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let too_big = "that share code expands to far more than a preset can be";
    let mut out = Vec::new();
    // Try raw DEFLATE first, then zlib-wrapped, because which one a producer
    // used is not visible from the bytes and guessing wrong is a hard error
    // rather than a wrong answer.
    if flate2::read::DeflateDecoder::new(data)
        .take(MAX_INFLATED + 1)
        .read_to_end(&mut out)
        .is_ok()
        && !out.is_empty()
    {
        if out.len() as u64 > MAX_INFLATED {
            return Err(too_big.into());
        }
        return Ok(out);
    }
    out.clear();
    flate2::read::ZlibDecoder::new(data)
        .take(MAX_INFLATED + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("that share code did not decompress: {e}"))?;
    if out.len() as u64 > MAX_INFLATED {
        return Err(too_big.into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(kind: &str, container: u32, kind_version: u32) -> String {
        format!(
            r#"{{"format":"coilbox","container":{container},"kind":"{kind}",
               "kindVersion":{kind_version},"payload":{{"a":1}}}}"#
        )
    }

    #[test]
    fn a_supported_container_identifies_as_readable() {
        let id = identify(&envelope("preset", 1, 1)).unwrap();
        assert_eq!(id.kind, "preset");
        assert_eq!(id.compatibility, Compatibility::Supported);
    }

    #[test]
    fn something_newer_is_refused_rather_than_half_read() {
        /* The property the two version numbers exist for. Reading a payload
           this build does not understand and silently dropping the half it
           cannot see is worse than saying no. */
        let newer_payload = identify(&envelope("preset", 1, 99)).unwrap();
        assert_eq!(newer_payload.compatibility, Compatibility::Newer);

        let newer_envelope = identify(&envelope("preset", 99, 1)).unwrap();
        assert_eq!(newer_envelope.compatibility, Compatibility::Newer);

        let err = open::<serde_json::Value>(&envelope("preset", 1, 99), "preset").unwrap_err();
        assert!(err.contains("newer Coilbox"), "{err}");
    }

    #[test]
    fn a_kind_we_have_no_reader_for_says_so_by_name() {
        let id = identify(&envelope("blueprint", 1, 1)).unwrap();
        assert_eq!(id.compatibility, Compatibility::UnknownKind);
        let err = open::<serde_json::Value>(&envelope("blueprint", 1, 1), "preset").unwrap_err();
        assert!(err.contains("blueprint"), "{err}");
    }

    #[test]
    fn a_file_that_is_not_a_container_is_refused_by_name() {
        let err = identify(r#"{"name":"my scenario","units":[]}"#).unwrap_err();
        assert!(err.contains("not a Coilbox file"), "{err}");
    }

    #[test]
    fn base64url_round_trips_every_length() {
        // The three chunk cases - 3 bytes, 2, 1 - and the alphabet's last two
        // entries, which are the ones that differ from standard base64.
        for bytes in [
            vec![0u8],
            vec![0, 255],
            vec![1, 2, 3],
            vec![0xff, 0xfe, 0xfd, 0xfc],
            (0u8..=255).collect::<Vec<u8>>(),
        ] {
            let text = encode_for_test(&bytes);
            assert_eq!(base64url_decode(&text).unwrap(), bytes, "{text}");
        }
    }

    #[test]
    fn a_share_code_reads_the_same_as_the_json() {
        use std::io::Write;
        let json = envelope("preset", 1, 1);
        let mut deflate =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflate.write_all(json.as_bytes()).unwrap();
        let code = format!("{CODE_PREFIX}{}", encode_for_test(&deflate.finish().unwrap()));

        let from_code = identify(&code).unwrap();
        assert_eq!(from_code, identify(&json).unwrap());
        assert_eq!(from_code.compatibility, Compatibility::Supported);
    }

    /// base64url without padding, for the tests only - nothing here writes codes.
    fn encode_for_test(data: &[u8]) -> String {
        const A: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = |i: usize| *chunk.get(i).unwrap_or(&0) as u32;
            let bits = (b(0) << 16) | (b(1) << 8) | b(2);
            for i in 0..chunk.len() + 1 {
                out.push(A[((bits >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        out
    }
}
