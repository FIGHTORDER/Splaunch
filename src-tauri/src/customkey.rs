//! Zero-K's "custom key" transport, and the two decoder faults it has.
//!
//! Zero-K passes structured mission data - objectives, features, terraform,
//! briefing text, placed units - through start script values, which can only
//! hold flat strings. Its encoding is a Lua table literal, base64'd:
//!
//! ```text
//! UsefulTableToCustomKey(t) = Base64Encode(TableToString(t))
//! CustomKeyToUsefulTable(s) = loadstring("return " .. Base64Decode(s:gsub('_','=')))
//! ```
//!
//! Two things in that pair are broken, and both were confirmed by porting the
//! Lua byte-for-byte and round-tripping payloads through it (see the tests):
//!
//! 1. The encoder is URL-safe base64, so sextet 63 emits `_`. The decoder
//!    rewrites `_` to `=` *before* decoding, and `=` is absent from its
//!    alphabet, so it reads as end-of-data. A `?` at the wrong offset silently
//!    truncates the payload - and a truncated Lua literal does not parse, so
//!    `CustomKeyToUsefulTable` returns nil and the mission loses every
//!    objective at once.
//! 2. The last byte of each triple is assembled with `... % 192`, which zeroes
//!    the top two bits when both are set. Any byte >= 0xC0 is corrupted, which
//!    is every UTF-8 lead byte - so accented and non-Latin text is mangled
//!    independently of fault 1.
//!
//! Both faults are properties of the *bytes on the wire*, so the fix is to put
//! nothing on the wire that can trigger them: every risky byte is written as a
//! Lua decimal escape (`\195`), which is plain ASCII in transport and becomes
//! the original byte again when Lua parses it. That makes the payload
//! transport-safe by construction rather than by luck, and it survives text in
//! any language.
//!
//! We deliberately do not "fix" Zero-K's decoder - it is the thing reading our
//! output, and it is not ours. We write what it can read correctly.

/// Zero-K's alphabet: URL-safe, with `=` used for padding.
const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// A Lua value, as far as this transport is concerned.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    Table(Table),
}

/// An ordered Lua table.
///
/// Ordered rather than a map because the output is compared in tests, and a
/// payload that reshuffles between runs is one nobody can diff.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Table {
    entries: Vec<(Key, Value)>,
}

#[derive(Debug, Clone, PartialEq)]
enum Key {
    Index(i64),
    Name(String),
}

impl Table {
    pub fn new() -> Self {
        Table { entries: Vec::new() }
    }

    /// `name = value`
    pub fn set(&mut self, name: &str, value: Value) -> &mut Self {
        self.entries.push((Key::Name(name.to_string()), value));
        self
    }

    /// `[i] = value`, for the 1-based arrays Zero-K indexes objectives by.
    pub fn set_index(&mut self, i: i64, value: Value) -> &mut Self {
        self.entries.push((Key::Index(i), value));
        self
    }

    /// Append at the next 1-based index.
    pub fn push(&mut self, value: Value) -> &mut Self {
        let next = self.entries.len() as i64 + 1;
        self.set_index(next, value)
    }
}

/// Convenience: `str_value("x")`, since `Value::Str("x".into())` reads badly in
/// a wall of objective fields.
pub fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

pub fn n(v: impl Into<f64>) -> Value {
    Value::Num(v.into())
}

pub fn b(v: bool) -> Value {
    Value::Bool(v)
}

pub fn t(v: Table) -> Value {
    Value::Table(v)
}

/// Is this byte safe to put on the wire unescaped?
///
/// `?` (0x3F) triggers fault 1, anything >= 0x80 triggers fault 2, and quote
/// and backslash would end or escape the Lua string. Control bytes are escaped
/// because a raw newline in a start script value would end the line.
fn safe_byte(byte: u8) -> bool {
    (0x20..0x7F).contains(&byte) && byte != b'?' && byte != b'"' && byte != b'\\'
}

/// A Lua string literal whose transport bytes cannot trip either decoder fault.
///
/// Escapes are always three digits, so a following digit cannot be absorbed
/// into the escape: Lua reads at most three, and `\0631` must stay `?` then
/// `1`.
fn lua_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for byte in value.as_bytes() {
        if safe_byte(*byte) {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("\\{:03}", byte));
        }
    }
    out.push('"');
    out
}

/// Numbers, the way Lua will read them back.
///
/// Integers are written without a decimal point because the game compares some
/// of these against integers, and `targetNumber=3.0` reading back as a float is
/// a difference nobody wants to debug through a base64 blob.
fn lua_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else if value.is_finite() {
        format!("{}", value)
    } else {
        // Lua would read `inf`/`nan` as a nil global and silently change the
        // table's shape, so refuse to emit one.
        "0".to_string()
    }
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Str(v) => out.push_str(&lua_string(v)),
        Value::Num(v) => out.push_str(&lua_number(*v)),
        Value::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        Value::Table(v) => write_table(out, v),
    }
}

fn write_table(out: &mut String, table: &Table) {
    out.push('{');
    for (key, value) in &table.entries {
        match key {
            Key::Index(i) => out.push_str(&format!("[{}]=", i)),
            Key::Name(name) => {
                out.push_str(name);
                out.push('=');
            }
        }
        write_value(out, value);
        out.push(',');
    }
    out.push('}');
}

/// The Lua literal, before base64. Exposed for tests and for showing an author
/// what their scenario compiles to.
pub fn to_lua(table: &Table) -> String {
    let mut out = String::new();
    write_table(&mut out, table);
    out
}

/// Zero-K's Base64Encode, transcribed.
fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let byte = |i: usize| -> u32 { *data.get(i).unwrap_or(&0) as u32 };
    let mut start = 0;
    while start < data.len() {
        let (b0, b1, b2) = (byte(start), byte(start + 1), byte(start + 2));
        let left = data.len() - start;
        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 % 4) << 4) | (b1 >> 4)) as usize] as char);
        out.push(if left > 1 {
            ALPHABET[(((b1 % 16) << 2) | (b2 >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if left > 2 { ALPHABET[(b2 % 64) as usize] as char } else { '=' });
        start += 3;
    }
    out
}

/// Encode a table for a start script value.
pub fn encode(table: &Table) -> String {
    base64_encode(to_lua(table).as_bytes())
}

#[cfg(test)]
/// Zero-K's Base64Decode *including* the `_` -> `=` substitution that
/// `CustomKeyToUsefulTable` applies first: this is what the game actually
/// runs, faults and all.
///
/// Nothing here decodes its own output to work out what it means, and nothing
/// should: this is the broken path, and depending on it for behaviour would be
/// building on the fault rather than around it. It exists to prove our output
/// survives it.
///
/// It briefly stopped being test-only, when packing a campaign used it to check
/// its payloads. That check reads the encoded text for a `_` directly now,
/// which is both the actual invariant and one the broken decoder cannot
/// express: a `_` past the first few characters decodes to a non-empty prefix,
/// so "did it come back empty" passes while the mission is still broken.
pub(crate) fn decode_as_the_game_does(encoded: &str) -> Vec<u8> {
    let swapped: Vec<u8> = encoded
        .bytes()
        .map(|c| if c == b'_' { b'=' } else { c })
        .collect();
    let index = |c: u8| -> Option<u32> {
        ALPHABET.iter().position(|a| *a == c).map(|p| p as u32)
    };
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < swapped.len() {
        let c: Vec<Option<u32>> = (0..4)
            .map(|i| swapped.get(pos + i).copied().and_then(index))
            .collect();
        let (c0, c1) = match (c[0], c[1]) {
            (Some(a), Some(b)) => (a, b),
            _ => break,
        };
        out.push((((c0 * 4) % 256) | (c1 / 16)) as u8);
        if let Some(c2) = c[2] {
            out.push((((c1 * 16) % 256) | ((c2 / 4) % 256)) as u8);
            if let Some(c3) = c[3] {
                // The `% 192` is Zero-K's, and is fault 2.
                out.push(((((c2 * 64) % 256) % 192) | c3) as u8);
            }
        }
        pos += 4;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn survives(table: &Table) -> bool {
        let lua = to_lua(table);
        super::decode_as_the_game_does(&encode(table)) == lua.as_bytes()
    }

    fn objective(description: &str) -> Table {
        let mut inner = Table::new();
        inner.set("description", s(description));
        inner.set("targetNumber", n(3));
        inner.set("comparisionType", n(1));
        let mut outer = Table::new();
        outer.push(t(inner));
        outer
    }

    #[test]
    fn plain_text_round_trips() {
        assert!(survives(&objective("Have 3 Glaives by 0:35.")));
    }

    #[test]
    fn a_question_mark_would_have_truncated_the_payload() {
        // Unescaped, this is the exact shape that silently loses objectives:
        // '?' at an offset that produces `_`, which the game reads as the end
        // of the data.
        let raw = b"{[1]={description=\"Can you hold the ridge?\",},}";
        let truncated = super::decode_as_the_game_does(&base64_encode(raw));
        assert_ne!(truncated, raw, "expected Zero-K's decoder to mangle this");

        // Escaped, the same sentence survives.
        assert!(survives(&objective("Can you hold the ridge?")));
    }

    #[test]
    fn non_ascii_survives() {
        for text in [
            "Halte den Grat fünf Minuten",
            "Продержись 5 минут",
            "守住山脊 5 分钟",
            "Tiens la crête — cinq minutes",
        ] {
            assert!(survives(&objective(text)), "lost: {text}");
        }
    }

    #[test]
    fn quotes_and_backslashes_survive() {
        assert!(survives(&objective(r#"He said "go?" then left \ north"#)));
    }

    #[test]
    fn every_byte_sequence_survives() {
        // Exhaustive over alignment: each risky byte at each offset in the
        // 3-byte grouping, which is what both faults actually key on.
        for byte in 0u32..=0x2FF {
            for pad in 0..3 {
                let text = format!(
                    "{}{}",
                    "x".repeat(pad),
                    char::from_u32(byte).unwrap_or('x')
                );
                assert!(survives(&objective(&text)), "lost byte {byte:#x} at pad {pad}");
            }
        }
    }

    #[test]
    fn escapes_cannot_absorb_a_following_digit() {
        // "?1" must not become the escape \0631.
        let mut table = Table::new();
        table.set("description", s("?1"));
        assert!(to_lua(&table).contains(r#""\0631""#));
        assert!(survives(&table));
    }

    #[test]
    fn integers_stay_integers() {
        let mut table = Table::new();
        table.set("targetNumber", n(3));
        table.set("satisfyByTime", n(35));
        assert_eq!(to_lua(&table), "{targetNumber=3,satisfyByTime=35,}");
    }

    #[test]
    fn shape_matches_zero_ks_own_serialiser() {
        // Compared against TableToString in luamenu/addons/tablefunctions.lua:
        // `[i]=` for numeric keys, bare name otherwise, trailing comma on every
        // entry, no whitespace.
        let mut inner = Table::new();
        inner.set("victoryByTime", n(50));
        inner.set("completeAllBonusObjectives", b(true));
        let mut outer = Table::new();
        outer.push(t(inner));
        assert_eq!(
            to_lua(&outer),
            "{[1]={victoryByTime=50,completeAllBonusObjectives=true,},}"
        );
    }

    #[test]
    fn base64_matches_a_known_value() {
        // Cross-checked against Python's urlsafe base64 for the same input,
        // since Zero-K's encoder is standard apart from its padding.
        assert_eq!(base64_encode(b"objective"), "b2JqZWN0aXZl");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"a"), "YQ==");
    }
}
