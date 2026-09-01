//! Reading a long text in bounded pieces: which piece a caller asked for, and where it falls.
//!
//! A reply that has to fit a ceiling cannot simply be cut short — the reader needs a way to ask
//! for the rest. A window is that ask: a chunk number, optionally narrowed to a range of lines.
//! Planning divides the lines it is given into consecutive chunks, each sized so its body fits
//! the capacity *once JSON has escaped it*, and reports where the requested chunk falls along
//! with the window that reaches the next one. Cost is measured escaped rather than raw because
//! the body travels inside a JSON string: a quote, a backslash, and a control character each
//! occupy more encoded than they do in memory, so a chunk measured raw would overrun the ceiling
//! it was sized to. Shared kernel — this depends on nothing, holds no state, reads nothing, and
//! knows neither who is reading nor what.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// The number of a text's first line: the line numbers a reader is given are 1-based.
const FIRST_LINE: u32 = 1;
/// The number of a plan's first chunk: chunk numbers, like line numbers, are 1-based.
const FIRST_CHUNK: u32 = 1;
/// What a character JSON writes as a two-character escape costs — `"`, `\`, and the control
/// characters that have a letter of their own.
const SHORT_ESCAPE_COST: usize = 2;
/// What a control character without a letter of its own costs, written `\u00XX`.
const UNICODE_ESCAPE_COST: usize = 6;
/// The last codepoint JSON escapes for being a control character. Above it, only `"` and `\`
/// are escaped; everything else is written as its own bytes.
const LAST_ESCAPED_CONTROL: char = '\u{1f}';
/// What the newline joining two pieces of one chunk costs against that chunk's capacity. The
/// body is measured as JSON will write it, and JSON writes a newline escaped.
const NEWLINE_COST: usize = escaped_char_cost('\n');
/// The bytes that same newline occupies unescaped, which is what a raw size counts.
const NEWLINE_BYTES: usize = '\n'.len_utf8();
/// The smallest capacity a chunk can make progress under. No character costs more than a
/// `\u00XX` escape, so beneath this a chunk that has to hold one has nowhere to put it and
/// planning would never advance past it.
const MIN_CAPACITY: usize = UNICODE_ESCAPE_COST;

/// What a caller asked for: which chunk, inside which absolute range of lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadWindow {
    /// The chunk wanted, 1-based.
    pub chunk: u32,
    /// The lines to plan over, or the whole text when absent.
    pub range: Option<LineRange>,
}

impl Default for ReadWindow {
    fn default() -> Self {
        Self {
            chunk: FIRST_CHUNK,
            range: None,
        }
    }
}

/// A span of lines, 1-based and inclusive at both ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    /// The first line of the span.
    pub from: u32,
    /// The last line of the span.
    pub to: u32,
}

/// Where one chunk falls, for the envelope the reply is delivered in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// Which chunk this is, 1-based.
    pub index: u32,
    /// How many chunks the planned range yields in total.
    pub of: u32,
    /// The absolute lines this chunk covers, the two it is cut within included.
    pub lines: LineRange,
    /// How many lines the whole text holds, whatever range was planned over.
    pub total_lines: u32,
    /// The raw UTF-8 bytes of this chunk's body text.
    pub bytes: usize,
    /// The raw UTF-8 bytes of the whole text, whatever range was planned over.
    pub total_bytes: usize,
    /// Which ends of this chunk fall inside a line rather than between two.
    pub cut: Cut,
    /// The window that reaches the next chunk, or `None` at the last one.
    pub next: Option<ReadWindow>,
}

/// Which ends of a chunk fall inside a line. A line too long for one chunk is cut, so a chunk
/// can begin or end part-way through one; a reader joining chunks back together needs to know
/// which joins take a newline and which do not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cut {
    /// The chunk begins part-way through its first line.
    pub starts_mid_line: bool,
    /// The chunk ends part-way through its last line.
    pub ends_mid_line: bool,
}

/// Why a window could not be planned.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum WindowError {
    /// The chunk asked for is not one the plan holds. Nothing is clamped: a caller asking past
    /// the end has lost track of where it is, and a silently repeated last chunk would read as
    /// progress.
    #[error("chunk {requested} was asked for, of {of}")]
    ChunkOutOfRange {
        /// The chunk the window named.
        requested: u32,
        /// How many chunks the plan holds.
        of: u32,
    },
    /// The range asked for is not one the text has: it starts before the first line, ends
    /// before it starts, or runs past the last line.
    #[error("lines {}-{} were asked for, of {}", .requested.from, .requested.to, .total_lines)]
    LineRangeOutOfRange {
        /// The range the window named.
        requested: LineRange,
        /// How many lines the text holds.
        total_lines: u32,
    },
    /// The text has no lines at all, so there is no chunk to return.
    #[error("there is nothing to read")]
    Empty,
    /// The capacity is too small for any chunk to make progress under.
    #[error("a capacity of {capacity} bytes cannot hold a chunk; {minimum} is the least that can")]
    NoCapacity {
        /// The capacity the caller offered.
        capacity: usize,
        /// The least capacity planning needs.
        minimum: usize,
    },
}

/// One slice of one input line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Which of the input lines this came from, indexed from zero.
    pub line: usize,
    /// The bytes of that line this covers — all of it unless the line had to be cut. Both ends
    /// fall on a character boundary.
    pub bytes: Range<usize>,
}

/// A planned chunk: where it falls, and the pieces its body is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    /// Where this chunk falls.
    pub chunk: Chunk,
    /// The pieces the body is made of, in order, joined by newlines.
    pub body: Vec<Piece>,
}

/// Plans every chunk of `lines` inside `window.range` so that each chunk's body costs at most
/// `capacity` bytes once JSON has escaped it, and returns the one `window.chunk` names.
///
/// Whole lines are taken while they fit; a line too long for a chunk of its own is cut on a
/// character boundary and continues in the next chunk. The plan is a function of the three
/// arguments alone, so asking for a later chunk of the same text sees the same division of it.
pub fn plan(lines: &[&str], window: &ReadWindow, capacity: usize) -> Result<Planned, WindowError> {
    if capacity < MIN_CAPACITY {
        return Err(WindowError::NoCapacity {
            capacity,
            minimum: MIN_CAPACITY,
        });
    }
    if lines.is_empty() {
        return Err(WindowError::Empty);
    }
    let total_lines = saturating_u32(lines.len());
    let range = window.range.unwrap_or(LineRange {
        from: FIRST_LINE,
        to: total_lines,
    });
    if range.from < FIRST_LINE || range.from > range.to || range.to > total_lines {
        return Err(WindowError::LineRangeOutOfRange {
            requested: range,
            total_lines,
        });
    }

    let first = (range.from - FIRST_LINE) as usize;
    let last = (range.to - FIRST_LINE) as usize;
    let mut planner = Planner::new(capacity);
    for (index, line) in lines.iter().enumerate().skip(first).take(last - first + 1) {
        planner.place(index, line);
    }
    let planned = planner.finish();

    let of = saturating_u32(planned.len());
    let body = window
        .chunk
        .checked_sub(FIRST_CHUNK)
        .and_then(|position| planned.into_iter().nth(position as usize))
        .ok_or(WindowError::ChunkOutOfRange {
            requested: window.chunk,
            of,
        })?;

    let ends = body.last();
    let chunk = Chunk {
        index: window.chunk,
        of,
        lines: LineRange {
            from: line_number(body.first.line),
            to: line_number(ends.line),
        },
        total_lines,
        bytes: joined_bytes(body.pieces().map(|piece| piece.bytes.len())),
        total_bytes: joined_bytes(lines.iter().map(|line| line.len())),
        cut: Cut {
            starts_mid_line: body.first.bytes.start != 0,
            ends_mid_line: lines
                .get(ends.line)
                .is_some_and(|line| ends.bytes.end != line.len()),
        },
        next: (window.chunk < of).then(|| ReadWindow {
            chunk: window.chunk + 1,
            range: window.range,
        }),
    };
    Ok(Planned {
        chunk,
        body: body.into_pieces(),
    })
}

/// What one character costs inside a JSON string, in bytes.
const fn escaped_char_cost(character: char) -> usize {
    match character {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => SHORT_ESCAPE_COST,
        '\0'..=LAST_ESCAPED_CONTROL => UNICODE_ESCAPE_COST,
        other => other.len_utf8(),
    }
}

/// `value` as a line or chunk number can name it, saturating rather than wrapping: a text with
/// more lines than a `u32` can number holds none a caller could ask for by number anyway.
fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// The 1-based number of the input line at `index`.
fn line_number(index: usize) -> u32 {
    saturating_u32(index).saturating_add(FIRST_LINE)
}

/// The raw bytes `parts` occupy once joined by newlines, as a body's text is.
fn joined_bytes(parts: impl Iterator<Item = usize>) -> usize {
    let (total, count) = parts.fold((0usize, 0usize), |(total, count), part| {
        (total + part, count + 1)
    });
    total + count.saturating_sub(1) * NEWLINE_BYTES
}

/// How much of a text fits within a given escaped cost.
enum Fit {
    /// All of it, at this cost.
    Whole(usize),
    /// Only its first `end` bytes, at this cost. The cut falls between two characters, so `end`
    /// is a character boundary.
    Prefix { end: usize, cost: usize },
}

/// How much of `text` fits within `room` bytes of escaped cost.
fn fitting_prefix(text: &str, room: usize) -> Fit {
    let mut cost = 0;
    for (at, character) in text.char_indices() {
        let with_character = cost + escaped_char_cost(character);
        if with_character > room {
            return Fit::Prefix { end: at, cost };
        }
        cost = with_character;
    }
    Fit::Whole(cost)
}

/// The pieces of one chunk, of which there is always at least one: a chunk is only ever closed
/// once something has gone into it.
struct Body {
    first: Piece,
    rest: Vec<Piece>,
}

impl Body {
    /// The last piece, which is the first one when nothing followed it.
    fn last(&self) -> &Piece {
        self.rest.last().unwrap_or(&self.first)
    }

    fn pieces(&self) -> impl Iterator<Item = &Piece> + '_ {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    fn into_pieces(self) -> Vec<Piece> {
        std::iter::once(self.first).chain(self.rest).collect()
    }
}

/// Fills chunks greedily: a chunk stays open until what comes next will not fit in it.
struct Planner {
    capacity: usize,
    closed: Vec<Body>,
    open: Option<Body>,
    cost: usize,
}

impl Planner {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            closed: Vec::new(),
            open: None,
            cost: 0,
        }
    }

    /// The escaped cost the open chunk can still take, the newline joining a further piece to
    /// what is already there included, or `None` when it can take nothing more.
    fn room(&self) -> Option<usize> {
        let join = if self.open.is_some() { NEWLINE_COST } else { 0 };
        self.capacity.checked_sub(self.cost)?.checked_sub(join)
    }

    fn push(&mut self, piece: Piece, cost: usize) {
        match &mut self.open {
            Some(body) => {
                body.rest.push(piece);
                self.cost += NEWLINE_COST + cost;
            }
            None => {
                self.open = Some(Body {
                    first: piece,
                    rest: Vec::new(),
                });
                self.cost = cost;
            }
        }
    }

    fn close(&mut self) {
        if let Some(body) = self.open.take() {
            self.closed.push(body);
            self.cost = 0;
        }
    }

    /// Places the whole of `line`, the input's `index`th, opening as many chunks as it takes.
    fn place(&mut self, index: usize, line: &str) {
        let mut placed = 0;
        loop {
            let Some(room) = self.room() else {
                self.close();
                continue;
            };
            match fitting_prefix(&line[placed..], room) {
                Fit::Whole(cost) => {
                    let piece = Piece {
                        line: index,
                        bytes: placed..line.len(),
                    };
                    self.push(piece, cost);
                    return;
                }
                Fit::Prefix { .. } if self.open.is_some() => self.close(),
                Fit::Prefix { end, cost } => {
                    let cut = placed + end;
                    let piece = Piece {
                        line: index,
                        bytes: placed..cut,
                    };
                    self.push(piece, cost);
                    self.close();
                    placed = cut;
                }
            }
        }
    }

    fn finish(mut self) -> Vec<Body> {
        self.close();
        self.closed
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
