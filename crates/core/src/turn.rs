//! The shape of a turn Soloist submits into an agent's terminal.
//!
//! Soloist writes into an agent's input on its own in two places — a fired timer waking its owner
//! and a worker reporting to its lead — and both deliver the same shape, differing only in the
//! header they carry. The shape belongs to neither, so it lives here: a submitted turn reads the
//! same whatever produced it, and the convention changes in one place.

/// The byte an agent CLI reads as "submit this turn" — the carriage return a terminal sends
/// for the Enter key.
const SUBMIT: u8 = b'\r';

/// Composes text Soloist delivers into an agent's terminal as a fresh submitted turn: a header
/// line saying where the turn came from, the body beneath it, and the byte that submits it —
/// without which the agent is left holding an unsent draft.
pub(crate) fn submitted_turn(header: &str, body: &str) -> Vec<u8> {
    let mut turn = format!("{header}\n{body}").into_bytes();
    turn.push(SUBMIT);
    turn
}
