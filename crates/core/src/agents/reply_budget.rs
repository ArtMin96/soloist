//! How much of a reply one MCP tool call may hand back.
//!
//! An agent CLI hosting an MCP session enforces its own ceiling on a tool result and cuts or
//! refuses whatever exceeds it, so a bulk reply is sized to the agent on the other end rather
//! than to however much data there happens to be. What each provider allows — the ceiling it
//! publishes, the environment variable its own user raises that ceiling with, and the
//! `tools/list` annotation it honours to lift one tool above the rest — is one row of the table
//! here, and every provider resolves through the same table. Reading the environment belongs to
//! the caller: [`resolve`] is handed a lookup and consults nothing itself.

use serde::Serialize;

use super::tool::AgentKind;

/// Bytes assumed per token where a provider states its ceiling in tokens. Tokenisation depends
/// on the text, so what this yields is a budget to stay under rather than a measurement.
pub const CHARS_PER_TOKEN: usize = 4;

/// The budget for a provider that publishes no ceiling, and for a session bound to no agent at
/// all. Equal to the strictest ceiling any provider is confirmed to impose, so a reply sized to
/// it fits every agent Soloist knows and every one it does not.
pub const DEFAULT_REPLY_BYTES: usize = 40_000;

/// The most bytes one MCP tool reply may total for a hosting agent, and how that figure was
/// reached. `bytes` counts the compact JSON of the whole reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ReplyBudget {
    /// The ceiling a reply is composed to stay under.
    pub bytes: usize,
    /// Where that ceiling came from.
    pub source: BudgetSource,
    /// The `tools/list` entry to advertise, where the provider honours one.
    pub annotation: Option<ListAnnotation>,
}

/// Where a [`ReplyBudget`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum BudgetSource {
    /// The hosting agent's own environment set the ceiling, through the variable its provider
    /// documents for that purpose.
    AgentEnv { var: &'static str },
    /// The provider's published ceiling, with nothing in the agent's environment overriding it.
    Provider(AgentKind),
    /// No provider is known for the session, so [`DEFAULT_REPLY_BYTES`] applies.
    Default,
}

/// A `tools/list` `_meta` entry a client honours to raise its inline ceiling. `key` is the
/// annotation the provider documents and `bytes` the budget it is told to allow — the same
/// figure replies are already sized to, so what is advertised and what is sent agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ListAnnotation {
    pub key: &'static str,
    pub bytes: usize,
}

/// One provider's reply ceiling: what it allows by default, how its own user raises that, and
/// which `tools/list` annotation it honours.
#[derive(Clone, Copy)]
struct ProviderRule {
    default_bytes: usize,
    env_override: Option<EnvOverride>,
    list_annotation: Option<&'static str>,
}

/// The environment variable a provider documents for raising its ceiling. Its value is a token
/// count — the unit every provider that publishes such a variable states it in.
#[derive(Clone, Copy)]
struct EnvOverride {
    var: &'static str,
}

/// The budget `stated` names, in bytes, or `None` when it is not a positive token count.
fn budget_from_tokens(stated: &str) -> Option<usize> {
    let tokens: usize = stated.parse().ok()?;
    if tokens == 0 {
        return None;
    }
    Some(tokens.saturating_mul(CHARS_PER_TOKEN))
}

/// Claude Code limits one tool result to 25,000 tokens, raises that for the whole session
/// through `MAX_MCP_OUTPUT_TOKENS`, and lifts a single tool above it through an
/// `anthropic/maxResultSizeChars` annotation on `tools/list`. — code.claude.com/docs/en/mcp and
/// code.claude.com/docs/en/env-vars, verified 2026-09-02.
const CLAUDE_CODE: ProviderRule = ProviderRule {
    default_bytes: 25_000 * CHARS_PER_TOKEN,
    env_override: Some(EnvOverride {
        var: "MAX_MCP_OUTPUT_TOKENS",
    }),
    list_annotation: Some("anthropic/maxResultSizeChars"),
};

/// Gemini CLI truncates tool output past `DEFAULT_TRUNCATE_TOOL_OUTPUT_THRESHOLD` characters,
/// and documents neither a variable to raise it nor an annotation to lift one tool above it. —
/// github.com/google-gemini/gemini-cli `packages/core/src/config/config.ts`, verified 2026-09-02.
const GEMINI_CLI: ProviderRule = ProviderRule {
    default_bytes: 40_000,
    env_override: None,
    list_annotation: None,
};

/// Kimi CLI caps tool output at `MCP_MAX_OUTPUT_CHARS` characters, with no variable to raise it
/// and no annotation to lift one tool above it. — github.com/MoonshotAI/kimi-cli
/// `src/kimi_cli/soul/toolset.py`, verified 2026-09-02.
const KIMI_CLI: ProviderRule = ProviderRule {
    default_bytes: 100_000,
    env_override: None,
    list_annotation: None,
};

/// The rule for a provider that publishes no ceiling Soloist can size to. Codex's effective
/// limit follows the model rather than a documented figure, and it truncates rather than
/// refuses; opencode imposes no client cap at all, so the default is what protects its context
/// instead; Amp and Copilot publish nothing; and a Generic tool is whatever the user configured.
/// [`DEFAULT_REPLY_BYTES`] is the honest answer for all of them — verified 2026-09-02.
const NO_PUBLISHED_CEILING: ProviderRule = ProviderRule {
    default_bytes: DEFAULT_REPLY_BYTES,
    env_override: None,
    list_annotation: None,
};

/// The reply ceiling for a provider — the single place that knows what each one allows.
fn rule_for(kind: AgentKind) -> ProviderRule {
    use AgentKind::*;
    match kind {
        Claude => CLAUDE_CODE,
        Gemini => GEMINI_CLI,
        Kimi => KIMI_CLI,
        Codex | Amp | OpenCode | Copilot | Generic => NO_PUBLISHED_CEILING,
    }
}

/// The reply budget for a session hosted by `provider`, resolved from that provider's rule and
/// the hosting agent's own environment.
///
/// `env` is the caller's lookup of that environment — the core reads none itself. A variable
/// that is unset, or holds anything other than a positive number, leaves the provider's own
/// ceiling in place rather than failing the reply: a budget is a figure to stay under, and a
/// mistyped override is not worth refusing to answer over.
pub fn resolve(provider: Option<AgentKind>, env: impl Fn(&str) -> Option<String>) -> ReplyBudget {
    let Some(kind) = provider else {
        return ReplyBudget {
            bytes: DEFAULT_REPLY_BYTES,
            source: BudgetSource::Default,
            annotation: None,
        };
    };
    let rule = rule_for(kind);
    let overridden = rule.env_override.and_then(|over| {
        let bytes = budget_from_tokens(&env(over.var)?)?;
        Some((bytes, BudgetSource::AgentEnv { var: over.var }))
    });
    let (bytes, source) = overridden.unwrap_or((rule.default_bytes, BudgetSource::Provider(kind)));
    ReplyBudget {
        bytes,
        source,
        annotation: rule
            .list_annotation
            .map(|key| ListAnnotation { key, bytes }),
    }
}

#[cfg(test)]
#[path = "reply_budget_tests.rs"]
mod tests;
