/// Whether a request left its progress opt-in unasked-for, so an unasked request stays exactly the
/// bytes it was before there was anything to ask for.
pub(super) fn not_asked_for(progress: &bool) -> bool {
    !progress
}

/// A spawn includes the compact Soloist orchestration instructions unless the caller opts out.
pub(super) const fn include_agent_instructions_by_default() -> bool {
    true
}

/// Whether the default-true spawn setting can be omitted from the wire.
pub(super) const fn is_default_include_agent_instructions(include: &bool) -> bool {
    *include
}
