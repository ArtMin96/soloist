//! The agent-facing usage guide for the Soloist MCP surface — the single source both the
//! `help` tool and the managed `AGENTS.md`/`CLAUDE.md` section render, so what an agent
//! reads in-band and what lives in a project file can never disagree.

use crate::identity::PROCESS_ID_ENV;

/// The guide text, as Markdown. Covers the mechanics an agent needs (identity and binding,
/// project scope, the trust gate, waking up without polling) and the coordination etiquette
/// that keeps concurrent agents from trampling each other's shared state.
pub fn agent_guide() -> String {
    format!(
        "\
This project runs under Soloist, a process supervisor that gives coding agents a shared,
project-scoped workspace over MCP (server: `soloist-mcp`, stdio).

### Identity and binding

- Soloist injects `{PROCESS_ID_ENV}` into every process it manages. When it is set, call
  `bind_session_process` with that id before anything else — binding attributes your locks,
  timers, and todo locks to your process and releases them when it closes.
- Outside a managed process, call `register_agent` with a label instead.
- `whoami` reports how you are bound and which project your tools act on.

### Project scope

- Tools act on your effective project: the one selected with `select_project`, the one your
  bound process runs in, or the sole open project. Scope never widens — a tool cannot touch
  another project.

### Tools and the trust gate

- Always available: project and process status/control, bulk commands, output reading and
  search, service ports, lease locks, and this setup group.
- Toggleable in Soloist's settings: scratchpads, todos, timers, key-value.
- Starting or restarting a command requires the user to have trusted it. An \"untrusted\"
  refusal means ask the user to trust the command in Soloist — never work around it.

### Wake up, don't poll

- Never busy-poll output or status in a loop. To act when other processes go quiet, arm
  `timer_fire_when_idle_any`/`timer_fire_when_idle_all`; to act later, `timer_set`; to wait
  for a server to come up, `wait_for_bound_port`. A fired timer delivers its body back to
  you as a fresh turn.

### Coordination etiquette

- Acquire a lease (`lock_acquire`) before editing state other agents may touch, and release
  it when done. Leases are signals with a TTL — they expire and auto-release when the
  holding process closes.
- Claim a todo with `todo_lock` before working on it, comment progress as you go, and
  `todo_complete` only once its blockers are done.
- Keep shared notes in scratchpads rather than ad-hoc files, and write with the revision
  you read — a conflict means someone edited first; re-read and retry, never clobber.
- Store small shared facts in the key-value store instead of re-deriving them from logs.\
"
    )
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
