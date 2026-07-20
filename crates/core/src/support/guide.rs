//! The agent-facing usage guide for the Soloist MCP surface, organized as a set of topics.
//!
//! The same topic set is the single source for three renderings, so what an agent reads can
//! never disagree across them: [`help_overview`] is the compact capability menu the `help` tool
//! returns with no argument; [`help_topic`] resolves one topic (by key or alias) for
//! `help(topic=…)`; and [`agent_guide`] concatenates every topic into the full document the
//! managed `AGENTS.md`/`CLAUDE.md` section carries. [`onboarding_hint`] is the short first-run
//! path the MCP server also advertises in its initialization instructions.

use crate::ids::PROCESS_ID_ENV;
use crate::settings::McpFeatureGroup;

/// What Soloist is — the one-paragraph framing that opens the full guide and every overview.
const INTRO: &str = "\
Soloist is a process supervisor that gives coding agents a shared, project-scoped workspace over
MCP (server `soloist-mcp`, stdio). Its tools let you see and control the processes in your project
and coordinate with the other agents working alongside you.";

/// The first-run path, single-sourced so the `help` overview and the MCP server's initialization
/// instructions advertise the same three steps.
const ONBOARDING: &str = "\
New here? Take these three steps first:
1. Call `whoami` — it reports which process you are bound to, who you are acting as, and which
   project your tools affect.
2. Call `help` with no topic for this capability overview.
3. Call `help` with a topic (for example `help(topic=\"timers\")`) when you need detail on one
   area. Topic aliases work too — `ports`, `services`, `status`, `how do I`, and `yaml` all
   route to the right place.";

/// One section of the guide: a stable `key`, the `aliases` that also resolve to it, a human
/// `title`, and the Markdown `body`. Built at call time because some bodies interpolate the
/// injected-id variable name or the toggleable-group labels; the renderings are cheap and rare
/// (a `help` call or a one-off file write), so there is no need to cache them.
struct GuideTopic {
    key: &'static str,
    aliases: &'static [&'static str],
    title: &'static str,
    body: String,
}

impl GuideTopic {
    /// Whether a normalized query names this topic — its key or any alias, compared under the
    /// same normalization so `select_project`, `select-project`, and `Select Project` all match.
    fn matches(&self, query: &str) -> bool {
        normalize(self.key) == query || self.aliases.iter().any(|alias| normalize(alias) == query)
    }

    /// The topic as a Markdown section — its title as an `###` heading over its body.
    fn rendered(&self) -> String {
        format!("### {}\n\n{}", self.title, self.body)
    }
}

/// Folds a topic key, alias, or user query to one comparable form: lowercased, with `-`/`_`
/// treated as spaces and runs of whitespace collapsed. So `how do I`, `how-do-i`, and `how_do_i`
/// are one token, and the alias table can be written in whichever form reads best.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c == '-' || c == '_' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The full topic set, in the order the overview lists them and the full guide renders them.
/// Adding an area of guidance is one entry here — the overview, the `help(topic=…)` lookup, and
/// the project-file section all pick it up.
fn topics() -> Vec<GuideTopic> {
    let toggleable = McpFeatureGroup::ALL
        .iter()
        .map(|group| group.label().to_lowercase())
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        GuideTopic {
            key: "getting-started",
            aliases: &["start", "how", "how do i", "begin", "overview", "workflow"],
            title: "Getting started",
            body: format!(
                "A typical session: you are bound to your process automatically (see `identity`), \
so call `whoami` to confirm your scope, read what is running with `list_processes` and \
`get_process_output`, coordinate through leases, todos, and scratchpads rather than ad-hoc \
files, and wait on other processes with idle timers instead of polling.\n\n{ONBOARDING}"
            ),
        },
        GuideTopic {
            key: "identity",
            aliases: &["binding", "whoami", "who", "bind", "register"],
            title: "Identity & binding",
            body: format!(
                "- When Soloist launches a process it injects `{PROCESS_ID_ENV}`. Your MCP session \
binds to that process **automatically when it connects** — you do not call anything, and there \
is no manual bind tool. Binding attributes your locks, timers, and todo locks to your process \
and releases them when it closes.\n\
- Call `whoami` to confirm how you are bound and which project your tools act on.\n\
- If Soloist did *not* launch you (no injected id), call `register_agent` with a label so \
`whoami` can report who is calling."
            ),
        },
        GuideTopic {
            key: "scope",
            aliases: &["project", "projects", "select_project", "effective-project"],
            title: "Project scope",
            body: "- Your tools act on your *effective project*: the one you set with \
`select_project`, the one your bound process runs in, or the sole open project. Scope never \
widens — a tool cannot touch another project.\n\
- Scope is proven by the process you run in. An agent launched *outside* Soloist while several \
projects are open has no project scope and cannot select one: use a global scope where a tool \
offers one (prompt templates), keep exactly one project open, or have Soloist launch you."
                .to_string(),
        },
        GuideTopic {
            key: "trust",
            aliases: &["untrusted", "trusted", "permission", "tools"],
            title: "The trust gate",
            body: format!(
                "- Always available: project and process status and control, bulk commands, output \
reading and search, service ports, lease locks, and this setup group.\n\
- Toggleable in Soloist's settings: {toggleable}. A disabled group's tools disappear from \
discovery entirely — they are neither listed nor callable.\n\
- Starting or restarting a command requires the user to have trusted it. An \"untrusted\" \
refusal means ask the user to trust the command in Soloist — never work around it."
            ),
        },
        GuideTopic {
            key: "status",
            aliases: &["output", "logs", "processes", "search"],
            title: "Reading status & output",
            body: "- `list_processes` and `get_process_status` show what exists and its state; \
`get_project_status` summarizes the project.\n\
- `get_process_output` returns the rendered terminal view; `get_process_raw_output` keeps the \
control sequences. `search_output`/`search_raw_output` scan the scrollback.\n\
- Do not poll these in a loop to wait for something — arm a timer instead (see `timers`)."
                .to_string(),
        },
        GuideTopic {
            key: "ports",
            aliases: &["port", "service", "services", "url"],
            title: "Service ports",
            body: "- `services_list` and `get_process_ports` report the localhost ports and URLs \
Soloist detected for running processes — the way to discover where a dev server is listening.\n\
- To act once a server is up, `wait_for_bound_port` blocks until the port binds instead of \
polling the log for a ready line."
                .to_string(),
        },
        GuideTopic {
            key: "timers",
            aliases: &["timer", "idle", "wait", "wake", "poll", "sleep"],
            title: "Wake up, don't poll",
            body: "- Never busy-poll output or status in a loop. To act when other processes go \
quiet, arm `timer_fire_when_idle_any` or `timer_fire_when_idle_all`; to act after a delay, \
`timer_set`; to wait for a server to come up, `wait_for_bound_port`.\n\
- A fired timer delivers its body back to you as a fresh turn, so you can hand off control and \
be woken exactly when there is something to do. `timer_list` shows what is armed."
                .to_string(),
        },
        GuideTopic {
            key: "coordination",
            aliases: &["etiquette", "concurrency", "cooperate"],
            title: "Coordination etiquette",
            body: "- Acquire a lease (`locks`) before editing state other agents may touch; claim \
a todo (`todos`) before working it; keep shared notes in scratchpads (`scratchpads`) rather than \
ad-hoc files; store small shared facts in the key-value store (`kv`) instead of re-deriving them \
from logs.\n\
- The theme throughout: signal your intent, write with the revision you read, and release what \
you hold when you are done."
                .to_string(),
        },
        GuideTopic {
            key: "locks",
            aliases: &["lock", "lease", "lock_acquire"],
            title: "Leases (advisory locks)",
            body: "- `lock_acquire` before editing state other agents may touch, and \
`lock_release` when done. Leases are signals with a TTL — they expire and auto-release when the \
holding process closes, so a crash never wedges the workspace.\n\
- `lock_status` shows who holds what. A lease is advisory: it coordinates willing agents, it does \
not forcibly block a write."
                .to_string(),
        },
        GuideTopic {
            key: "scratchpads",
            aliases: &["scratchpad", "notes", "docs"],
            title: "Shared scratchpads",
            body: "- Scratchpads are durable, project-scoped shared documents — a free-form \
Markdown note addressed by its name. Write whatever structure the work needs; a project template \
can seed a starting shape.\n\
- `scratchpad_read` returns the body *and* its revision; pass that revision back to \
`scratchpad_write` to update it. A revision mismatch means someone edited first — re-read and \
retry, never clobber."
                .to_string(),
        },
        GuideTopic {
            key: "todos",
            aliases: &["todo", "tasks", "task"],
            title: "Shared todos",
            body: "- Todos are the shared task list. Claim one with `todo_lock` before working it \
so two agents do not duplicate the effort, comment progress as you go, and `todo_complete` only \
once its blockers are done.\n\
- A todo lock is owned by your process and releases when it closes, so an abandoned claim frees \
itself."
                .to_string(),
        },
        GuideTopic {
            key: "kv",
            aliases: &["key-value", "keyvalue", "state"],
            title: "Key-value store",
            body: "- The key-value store holds small, structured shared facts as JSON — a chosen \
port, a resolved config value, a decision — so agents read them instead of re-deriving them from \
logs.\n\
- It is for small state, not logs or long text; put prose in a scratchpad. This group is off by \
default and enabled in Soloist's settings."
                .to_string(),
        },
        GuideTopic {
            key: "prompt-templates",
            aliases: &["prompt", "prompts", "template", "templates"],
            title: "Prompt templates",
            body: "- Prompt templates are durable, reusable prompt bodies with `{{name}}` \
fill-ins. `prompt_template_list`, `_read`, `_create`, `_update`, `_delete` and `_export` manage \
them; `prompt_template_render` hands back one with your values substituted. Updates are \
revision-guarded like scratchpads — write with the revision you read.\n\
- A template lives in your effective project (the default) or in the `global` scope shared across \
projects, and the same name may exist in both. A `list` with no scope merges the two.\n\
- A single backslash before `{{` escapes the marker and is consumed, so `\\{{x}}` is the literal \
text `{{x}}` and declares no placeholder. A doubled backslash is one literal backslash and escapes \
nothing; a longer run pairs off, with an odd one left over escaping.\n\
- Render with a `values` map. A placeholder you supply no value for is left in the text as-is and \
named in `unfilled`; a value naming no placeholder is named in `unknown`. Substituted text is \
never rescanned, so a value containing `{{a}}` stays literal. This group is off by default and \
enabled under Integrations in Soloist's settings."
                .to_string(),
        },
        GuideTopic {
            key: "yaml",
            aliases: &["config", "solo.yml", "solo-yml"],
            title: "solo.yml configuration",
            body: "- `solo.yml` at the project root defines the processes Soloist supervises: a \
map keyed by process name, each with a `command` and optional `working_dir`, `auto_start`, \
`auto_restart`, `restart_when_changed` globs, and per-process `env`.\n\
- The user owns this file; Soloist never rewrites it silently. Changes are picked up and the \
affected commands restart. You cannot start a command the user has not trusted."
                .to_string(),
        },
    ]
}

/// The full guide as Markdown: the intro followed by every topic as a section. This is what the
/// managed `AGENTS.md`/`CLAUDE.md` section carries, so a project file and the in-band `help`
/// answers are the same content.
pub fn agent_guide() -> String {
    let mut guide = String::from(INTRO);
    for topic in topics() {
        guide.push_str("\n\n");
        guide.push_str(&topic.rendered());
    }
    guide
}

/// The compact capability overview the `help` tool returns with no topic: the intro, the
/// first-run path, and a one-line menu of the topics an agent can ask about.
pub fn help_overview() -> String {
    let menu = topics()
        .iter()
        .map(|topic| format!("- `{}` — {}", topic.key, topic.title))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{INTRO}\n\n{ONBOARDING}\n\nTopics — call `help` with any of these:\n{menu}")
}

/// The rendered section for the topic `query` names by key or alias, or `None` when nothing
/// matches (the caller then falls back to the overview).
pub fn help_topic(query: &str) -> Option<String> {
    let query = normalize(query);
    topics()
        .into_iter()
        .find(|topic| topic.matches(&query))
        .map(|topic| topic.rendered())
}

/// The short first-run path the MCP server advertises in its initialization instructions —
/// the same three steps the overview opens with.
pub fn onboarding_hint() -> &'static str {
    ONBOARDING
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
