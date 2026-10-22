//! Splits an rAthena NPC source file into header records, skipping comments and `{...}` script
//! bodies (spec TOOL-NPC-02; grammar in `reference/catalog/db/npc_files.md` §2).

use std::fmt;

/// One header line split on tabs. `w4` holds the rest of the line, further tabs included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// 1-based line of the header.
    pub line: usize,
    pub w1: String,
    pub w2: String,
    pub w3: String,
    pub w4: String,
}

/// A problem that makes rAthena abandon the rest of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    Bom,
    TooFewFields { line: usize },
    MissingBody { line: usize },
    UnterminatedBody { line: usize },
    NewlineInString { line: usize },
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bom => write!(f, "UTF-8 BOM at start of file"),
            Self::TooFewFields { line } => write!(f, "line {line}: fewer than 3 tab fields"),
            Self::MissingBody { line } => write!(f, "line {line}: script without ',{{'"),
            Self::UnterminatedBody { line } => write!(f, "line {line}: unterminated script body"),
            Self::NewlineInString { line } => write!(f, "line {line}: newline inside a string"),
        }
    }
}

/// All records up to the first fatal error (if any), like rAthena's `npc_parsesrcfile`.
pub fn records(bytes: &[u8]) -> (Vec<Record>, Option<LexError>) {
    let mut out = Vec::new();
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (out, Some(LexError::Bom));
    }
    let mut lines = LineCounter::default();
    let mut pos = 0;
    loop {
        pos = skip_space(bytes, pos);
        if pos >= bytes.len() {
            return (out, None);
        }
        let line = lines.line_at(bytes, pos);
        let line_end = memchr(b'\n', bytes, pos).unwrap_or(bytes.len());
        let mut header = &bytes[pos..line_end];
        if header.last() == Some(&b'\r') && line_end < bytes.len() {
            header = &header[..header.len() - 1];
        }
        let text = decode(header);
        let mut fields = text.splitn(4, '\t');
        let (Some(w1), Some(w2), Some(w3)) = (fields.next(), fields.next(), fields.next()) else {
            return (out, Some(LexError::TooFewFields { line }));
        };
        let w4 = fields.next().unwrap_or("");
        let is_script = w2
            .get(..6)
            .is_some_and(|p| p.eq_ignore_ascii_case("script"));
        let next = if is_script {
            let header_bytes = &bytes[pos..line_end];
            let open = if w1.eq_ignore_ascii_case("function") {
                find(header_bytes, b"{")
            } else {
                find(header_bytes, b",{").map(|i| i + 1)
            };
            let Some(open) = open else {
                return (out, Some(LexError::MissingBody { line }));
            };
            match skip_body(bytes, pos + open) {
                Ok(end) => end,
                Err(BodyError::Unterminated) => {
                    return (out, Some(LexError::UnterminatedBody { line }))
                }
                Err(BodyError::NewlineInString) => {
                    return (out, Some(LexError::NewlineInString { line }))
                }
            }
        } else {
            line_end
        };
        out.push(Record {
            line,
            w1: w1.to_string(),
            w2: w2.to_string(),
            w3: w3.to_string(),
            w4: w4.to_string(),
        });
        pos = next;
    }
}

/// UTF-8, or CP949 (EUC-KR superset) for the few Korean headers that aren't UTF-8.
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::EUC_KR.decode(bytes).0.into_owned(),
    }
}

/// rAthena `skip_space`: whitespace, `// ...` to end of line, and `/* ... */` (may span lines;
/// unterminated consumes the rest of the file).
fn skip_space(bytes: &[u8], mut pos: usize) -> usize {
    loop {
        while pos < bytes.len() && is_space(bytes[pos]) {
            pos += 1;
        }
        let rest = &bytes[pos.min(bytes.len())..];
        if rest.starts_with(b"//") {
            pos = memchr(b'\n', bytes, pos).unwrap_or(bytes.len());
        } else if rest.starts_with(b"/*") {
            pos = find(&bytes[pos + 2..], b"*/").map_or(bytes.len(), |i| pos + 2 + i + 2);
        } else {
            return pos;
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

enum BodyError {
    Unterminated,
    NewlineInString,
}

/// rAthena `npc_skip_script` from the opening `{`: returns the position after the matching `}`.
/// Comments are skipped, `"` strings honour `\` escapes (unless the byte before the `\` is a
/// multibyte lead, >= 0x7f), single quotes are not special.
fn skip_body(bytes: &[u8], open: usize) -> Result<usize, BodyError> {
    let mut depth = 0usize;
    let mut p = open;
    loop {
        p = skip_space(bytes, p);
        let Some(&c) = bytes.get(p) else {
            return Err(BodyError::Unterminated);
        };
        match c {
            b'{' => {
                depth += 1;
                p += 1;
            }
            b'}' => {
                p += 1;
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(p);
                }
            }
            b'"' => {
                p += 1;
                loop {
                    let Some(&s) = bytes.get(p) else {
                        return Err(BodyError::Unterminated);
                    };
                    match s {
                        b'"' => {
                            p += 1;
                            break;
                        }
                        b'\n' => return Err(BodyError::NewlineInString),
                        b'\\' if !(p > 0 && bytes[p - 1] >= 0x7f) => p += 2,
                        _ => p += 1,
                    }
                }
            }
            _ => p += 1,
        }
    }
}

fn memchr(needle: u8, bytes: &[u8], from: usize) -> Option<usize> {
    bytes
        .get(from..)?
        .iter()
        .position(|&b| b == needle)
        .map(|i| from + i)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Converts byte offsets to 1-based line numbers; offsets must be asked in increasing order.
#[derive(Default)]
struct LineCounter {
    pos: usize,
    line: usize,
}

impl LineCounter {
    fn line_at(&mut self, bytes: &[u8], target: usize) -> usize {
        let end = target.min(bytes.len());
        if end > self.pos {
            self.line += bytes[self.pos..end].iter().filter(|&&b| b == b'\n').count();
            self.pos = end;
        }
        self.line + 1
    }
}
