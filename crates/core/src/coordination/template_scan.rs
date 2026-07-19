//! The placeholder tokenizer: the one scan of a template body, shared by every reader of it.
//!
//! A body is literal text interleaved with `{{name}}` markers. Naming the placeholders a body
//! declares and substituting values into it are two views of the *same* token stream, so a name
//! that is reported can never be one that substitution misses, and an escaped marker is invisible
//! to both. A second scanner would be free to disagree; there is only this one.
//!
//! Values are emitted exactly as given — never HTML-escaped. A template body carries code, and
//! escaping would corrupt the angle brackets, ampersands, and quotes that are its payload.
//! Substituted text is likewise never rescanned: a value containing `{{other}}` is literal.

/// Opens a placeholder marker.
const PLACEHOLDER_OPEN: &str = "{{";

/// Closes a placeholder marker.
const PLACEHOLDER_CLOSE: &str = "}}";

/// Suppresses the marker that immediately follows it.
const PLACEHOLDER_ESCAPE: char = '\\';

/// One piece of a scanned body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Token<'a> {
    /// Text to emit unchanged.
    Literal(&'a str),
    /// A well-formed marker: `name` is the trimmed inner text and `raw` the whole `{{…}}` span, so
    /// a reader with no value for the name can emit the marker untouched.
    Placeholder { name: &'a str, raw: &'a str },
    /// An escaped marker: the backslash is consumed and the `{{` that followed it is literal text.
    EscapedOpen,
}

impl<'a> Token<'a> {
    /// The text this token stands for when nothing is substituted into it: a literal's own text, a
    /// placeholder's whole `{{…}}` span, and the marker an escape suppressed.
    ///
    /// Emitting every token this way reproduces the body with its escapes resolved, so a reader
    /// that fills only the placeholders it has values for never has to know how a marker or an
    /// escape is spelled — that stays here, with the grammar that defines it.
    pub(super) fn verbatim(self) -> &'a str {
        match self {
            Token::Literal(text) => text,
            Token::Placeholder { raw, .. } => raw,
            Token::EscapedOpen => PLACEHOLDER_OPEN,
        }
    }
}

/// Scans `body` left to right into its tokens.
///
/// The first `}}` closes a candidate and inner text is trimmed, so `{{ name }}` names `name`. A
/// candidate that trims to empty or still contains a brace or a newline is not a placeholder — its
/// whole span is literal text and is not rescanned. A `{{` with no `}}` after it is literal to the
/// end of the body.
///
/// A single escaping backslash immediately before `{{` suppresses the marker and is consumed; a
/// pair of them is one literal backslash and suppresses nothing, so `\{{x}}` is the literal text
/// `{{x}}` while `\\{{x}}` is a backslash followed by a real placeholder. Backslashes anywhere else
/// are untouched — this is not a general-purpose escape character.
pub(super) fn scan(body: &str) -> impl Iterator<Item = Token<'_>> {
    Scan {
        rest: body,
        pending: None,
    }
}

/// The scan's position: the text still to read, plus the token a step produced behind a literal
/// prefix it had to emit first.
struct Scan<'a> {
    rest: &'a str,
    pending: Option<Token<'a>>,
}

impl<'a> Iterator for Scan<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(token) = self.pending.take() {
            return Some(token);
        }
        if self.rest.is_empty() {
            return None;
        }
        let (prefix, token, rest) = step(self.rest);
        self.rest = rest;
        if prefix.is_empty() {
            return Some(token);
        }
        self.pending = Some(token);
        Some(Token::Literal(prefix))
    }
}

/// Reads one marker out of `rest`, returning the literal text before it, the token it produced, and
/// the text still to scan.
///
/// Every index here comes from a match on an ASCII pattern or a count of ASCII backslashes, so each
/// slice lands on a character boundary whatever the body's encoding.
fn step(rest: &str) -> (&str, Token<'_>, &str) {
    let Some(open) = rest.find(PLACEHOLDER_OPEN) else {
        return ("", Token::Literal(rest), "");
    };
    let escapes = rest[..open]
        .chars()
        .rev()
        .take_while(|character| *character == PLACEHOLDER_ESCAPE)
        .count();
    // Each pair of backslashes collapses to one literal backslash; an odd one out escapes the
    // marker and is consumed. The prefix therefore keeps the text before the run plus that half.
    let prefix = &rest[..open - escapes + escapes / 2];
    if escapes % 2 == 1 {
        return (
            prefix,
            Token::EscapedOpen,
            &rest[open + PLACEHOLDER_OPEN.len()..],
        );
    }

    let inner = &rest[open + PLACEHOLDER_OPEN.len()..];
    let Some(close) = inner.find(PLACEHOLDER_CLOSE) else {
        return (prefix, Token::Literal(&rest[open..]), "");
    };
    let raw = &rest[open..open + PLACEHOLDER_OPEN.len() + close + PLACEHOLDER_CLOSE.len()];
    let remaining = &inner[close + PLACEHOLDER_CLOSE.len()..];
    let name = inner[..close].trim();
    if name.is_empty() || name.contains(['{', '}', '\n']) {
        (prefix, Token::Literal(raw), remaining)
    } else {
        (prefix, Token::Placeholder { name, raw }, remaining)
    }
}

#[cfg(test)]
#[path = "template_scan_tests.rs"]
mod tests;
