//! The caching hints this server's list results carry.
//!
//! From protocol version `2026-07-28` on, a complete list result must carry `ttlMs` and
//! `cacheScope`; peers on older versions have no schema for either field. rmcp models both as
//! optional because it serializes results for either era, and fills them in only inside the
//! `list_tools`/`list_prompts` its handler macros generate — which it skips for a server that
//! writes those two itself, as this one does to order the featured tools first and to gate the
//! prompts primitive. A peer on the newer version rejects the whole response when the fields are
//! missing, so this is where both lists get them.

use rmcp::model::{CacheScope, ListPromptsResult, ListToolsResult, ProtocolVersion};

/// The first protocol version whose list results carry caching hints.
const HINTS_SINCE: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// How long a client may treat one of these lists as fresh: not at all.
///
/// Neither list can be promised to hold still. The served tools are composed once, at connection
/// time, from the user's feature settings, and this server cannot say when those change; the
/// prompts follow templates the user can also edit in the app, where nothing raises
/// `listChanged`. Both are cheap to re-read, so a caller that needs one reads it again.
const TTL_MS: u64 = 0;

/// Who may reuse one of these lists: only the caller that asked for it.
///
/// Neither list is the same for every user — the tools are the ones this user's feature groups
/// enable, the prompts are their project's templates — so a shared cache must never serve one
/// install's surface to another.
const SCOPE: CacheScope = CacheScope::Private;

/// Stamps this server's caching hints onto a list result.
pub(crate) trait WithCacheHints: Sized {
    /// Sets `ttlMs` and `cacheScope` when the peer negotiated a version that requires them, and
    /// leaves the result alone otherwise, which is the shape an older peer expects.
    fn with_cache_hints(self, negotiated: Option<ProtocolVersion>) -> Self;
}

/// Whether the negotiated version's list results carry caching hints. Dated versions order
/// lexically as they do chronologically, so anything from [`HINTS_SINCE`] on qualifies.
fn hints_expected(negotiated: Option<ProtocolVersion>) -> bool {
    negotiated.is_some_and(|version| version >= HINTS_SINCE)
}

/// Implements [`WithCacheHints`] for each list result, so the two cannot drift apart and a third
/// joins them by name alone.
macro_rules! stamps_cache_hints {
    ($($result:ty),+ $(,)?) => {$(
        impl WithCacheHints for $result {
            fn with_cache_hints(mut self, negotiated: Option<ProtocolVersion>) -> Self {
                if hints_expected(negotiated) {
                    self.ttl_ms = Some(TTL_MS);
                    self.cache_scope = Some(SCOPE);
                }
                self
            }
        }
    )+};
}

stamps_cache_hints!(ListToolsResult, ListPromptsResult);

#[cfg(test)]
#[path = "cache_hints_tests.rs"]
mod tests;
