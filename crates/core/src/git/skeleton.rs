//! The starting shape a pull-request description is written into, and when there is one worth
//! writing into.
//!
//! Two things produce a description — the prompt an agent is asked to answer, and the suggestion
//! computed from the branch's own commits — and both fill the same shape, so what counts as one is
//! decided in one place. Past the ceiling a shape has stopped being a shape, and it is dropped
//! whole rather than cut: half a skeleton is filled in as though it were the whole of one.

/// The longest description template that is worth using as a shape. Generous for the handful of
/// headings a template actually is, and a ceiling on a document somebody filed as one.
const SKELETON_LIMIT: usize = 8 * 1024;

/// The shape to fill, or `None` where there is none worth filling: nothing was supplied, or what
/// was supplied is past the ceiling and would arrive as a fragment.
pub(super) fn shape(skeleton: &str) -> Option<&str> {
    (!skeleton.trim().is_empty() && skeleton.len() <= SKELETON_LIMIT).then_some(skeleton)
}
