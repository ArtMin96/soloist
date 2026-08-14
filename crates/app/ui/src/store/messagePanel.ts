// Pure helpers for the messages panel. No IPC, no state — only derivations over the
// `AgentMessageRecord` values on the orchestration snapshot, so these are straightforwardly
// testable.

import type {
  AgentMessageKind,
  AgentMessageOutcome,
  AgentMessageRecord,
  AgentNode,
} from "@/domain";

/**
 * Retained transcript entries per project. Mirrors `MAX_TRANSCRIPT_ENTRIES_PER_PROJECT` in the
 * core — the one place this side names the ceiling, so the head-of-list notice tells the truth
 * about why a conversation begins where it does.
 */
export const MAX_RETAINED_MESSAGES = 512;

/**
 * How many of the most recent exchanges render before the reader asks for the rest. A transcript
 * reads newest-last and a glance wants recent traffic, so the resting DOM stays small however
 * chatty the run — the bounded stand-in for virtualising a list that is already capped.
 */
export const VISIBLE_MESSAGE_WINDOW = 80;

/** The human label for why an addressed message exists. */
export const MESSAGE_KIND: Record<AgentMessageKind, string> = {
  direct: "Direct",
  task: "Task",
  completion: "Completion",
};

/**
 * The human label for how far a message has travelled. Worded as the reader sees it rather than
 * as the core names it: a submitted wake is a message that reached the agent's CLI, and an
 * acknowledgement is the agent accepting it.
 */
export const MESSAGE_OUTCOME: Record<AgentMessageOutcome, string> = {
  queued: "Queued",
  wake_submitted: "Delivered",
  acknowledged: "Accepted",
};

/**
 * Resolves process ids to their display labels from the snapshot's agent list. A sender or
 * recipient that has left the registry keeps its id, so a closed worker's messages stay readable
 * rather than rendering a blank.
 */
export function agentLabels(agents: AgentNode[]): (id: number) => string {
  const byId = new Map(agents.map((agent) => [agent.id, agent.label]));
  return (id) => byId.get(id) ?? `#${id}`;
}

/**
 * Whether the retained log is at its ceiling, so the conversation on screen may begin partway
 * through. A transcript that silently starts mid-exchange misleads; the panel says so instead.
 */
export function isAtRetentionCeiling(messages: AgentMessageRecord[]): boolean {
  return messages.length >= MAX_RETAINED_MESSAGES;
}

/**
 * The most recent exchanges, oldest-first within the window, plus how many older ones the window
 * is holding back. Expanding renders the whole retained log.
 */
export function windowMessages(
  messages: AgentMessageRecord[],
  expanded: boolean,
): { visible: AgentMessageRecord[]; hidden: number } {
  if (expanded || messages.length <= VISIBLE_MESSAGE_WINDOW) {
    return { visible: messages, hidden: 0 };
  }
  return {
    visible: messages.slice(messages.length - VISIBLE_MESSAGE_WINDOW),
    hidden: messages.length - VISIBLE_MESSAGE_WINDOW,
  };
}

/** The wall-clock time an exchange was recorded, in the reader's own locale. */
export function formatMessageTime(atUnixMillis: number): string {
  return new Date(atUnixMillis).toLocaleTimeString();
}
