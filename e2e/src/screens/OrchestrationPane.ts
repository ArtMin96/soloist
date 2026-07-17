import type { AgentActivity, ProcessKind } from "@domain";
import { $, browser } from "@wdio/globals";
import { WAIT } from "../harness/waits.js";
import { ROW_ACTIVITY, ROW_TEXT } from "./indicatorRow.js";

// The agent lineage tree the orchestration pane renders: a `role="tree"` of `role="treeitem"`
// nodes, workers nested under their lead inside a `role="group"`. Every handle here reads the tree's
// real ARIA semantics (roles, `aria-level`) plus the one machine attribute each node already carries
// for its own reasons (`data-process-id`, the ephemeral id; `data-activity`, the live glyph state).
// A node renders the shared indicator-row markup (`indicatorRow.ts`): its text spans are the name
// then the kind, and a node not reporting activity carries `data-status` instead (or nothing while
// resting), so a missing activity reads as `null`, never a wrong value.
const TREE = '[role="tree"][aria-label="Agent lineage"]';
const NODE = '[role="treeitem"]';
const GROUP = '[role="group"]';

// The pane's view switch (a segmented control), and the label each view segment renders. Named for
// what the user reads, so a spec asks for a view and never a label string.
const VIEW_SWITCH = '[aria-label="Orchestration views"]';
const VIEW_LABEL = {
  agents: "Agents",
  todos: "To-dos",
  scratchpads: "Scratchpads",
  timers: "Timers",
} as const;

/** The views the orchestration pane switches between. */
export type OrchestrationView = keyof typeof VIEW_LABEL;

/** One node of the orchestration tree, read from its real ARIA semantics. */
export interface TreeNode {
  /** The ephemeral process id the node carries — stable within a run, unique across nodes. */
  id: number;
  /** The row's name — the process's label. */
  label: string;
  kind: ProcessKind;
  /** The nesting depth ARIA reports: 1 for a root, 2 for a worker under its lead. */
  level: number;
  /** The live 5-state activity when the node reports one, else `null` (not a running tracked agent). */
  activity: AgentActivity | null;
  /** The id of the node this one nests under (its lead), or `null` when it is a root. */
  parent: number | null;
}

/** The raw per-node attributes read from the DOM, before they are coerced to typed values. */
interface RawNode {
  id: string | null;
  label: string;
  kind: string;
  level: string | null;
  activity: string | null;
  parent: string | null;
}

/** The project's live orchestration surface: the agent lineage tree and its live activity. */
export const orchestrationPane = {
  /** Waits for the tree to render — the pane has switched to the agents view and has agents. */
  async waitForTree(): Promise<void> {
    await $(TREE).waitForDisplayed({ timeout: WAIT.core });
  },

  /**
   * Switches the pane to `view` by clicking its segment, the way a user does. The segments are a
   * Radix ToggleGroup (each a real button that toggles on click, unlike the DropdownMenu the pane
   * is opened from), so a classic-WebDriver click selects one. The body swaps to the chosen view;
   * the caller waits on that view's own landmark.
   */
  async showView(view: OrchestrationView): Promise<void> {
    const segment = await $(VIEW_SWITCH).$(`button=${VIEW_LABEL[view]}`);
    await segment.waitForClickable({ timeout: WAIT.render });
    await segment.click();
  },

  /**
   * Every tree node currently rendered, read in one pass.
   *
   * Read atomically rather than node-by-node: a live worker re-renders its row as its activity
   * changes and a closed lead restructures the tree, so walking the nodes one driver call at a time
   * races the re-render and dies on a stale element reference. One snapshot cannot tear.
   */
  async nodes(): Promise<TreeNode[]> {
    const raw: RawNode[] = await browser.execute(
      (treeSel: string, nodeSel: string, groupSel: string, activitySel: string, textSel: string) => {
        const tree = document.querySelector(treeSel);
        if (!tree) return [];
        return [...tree.querySelectorAll(nodeSel)].map((node) => {
          const texts = [...node.querySelectorAll(textSel)].map(
            (span) => (span as HTMLElement).textContent?.trim() ?? "",
          );
          // A node inside a group nests under that group's owning row — the treeitem that is the
          // group's sibling under their shared Collapsible parent. Resolve the parent's id
          // structurally, so the read reflects the real nesting rather than a coincidence of order.
          let parent: string | null = null;
          const group = node.closest(groupSel);
          const owner = group?.parentElement?.querySelector(`:scope > ${nodeSel}`);
          if (owner) parent = owner.getAttribute("data-process-id");
          return {
            id: node.getAttribute("data-process-id"),
            label: texts[0] ?? "",
            kind: texts[1] ?? "",
            level: node.getAttribute("aria-level"),
            activity: node.querySelector(activitySel)?.getAttribute("data-activity") ?? null,
            parent,
          };
        });
      },
      TREE,
      NODE,
      GROUP,
      ROW_ACTIVITY,
      ROW_TEXT,
    );
    return raw.map((node) => ({
      id: Number(node.id),
      label: node.label,
      kind: node.kind as ProcessKind,
      level: Number(node.level),
      activity: node.activity as AgentActivity | null,
      parent: node.parent === null ? null : Number(node.parent),
    }));
  },

  /** Waits until at least one node labelled `label` is rendered, returning every such node. */
  async waitForNodes(label: string): Promise<TreeNode[]> {
    let found: TreeNode[] = [];
    let seen: string[] = [];
    try {
      await browser.waitUntil(
        async () => {
          const nodes = await this.nodes();
          seen = nodes.map((node) => node.label);
          found = nodes.filter((node) => node.label === label);
          return found.length > 0;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `no orchestration node labelled "${label}" appeared; rendered nodes: ${JSON.stringify(seen)}`,
      );
    }
    return found;
  },

  /** Waits until the node labelled `label` reports `activity` — a real idle-FSM transition. */
  async waitForActivity(label: string, activity: AgentActivity): Promise<void> {
    let last: AgentActivity | null | undefined;
    try {
      await browser.waitUntil(
        async () => {
          const node = (await this.nodes()).find((candidate) => candidate.label === label);
          last = node?.activity;
          return last === activity;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `orchestration node "${label}" never reported activity "${activity}"; last seen: ${
          last ?? "no such node / no activity"
        }`,
      );
    }
  },

  /**
   * Waits until the node labelled `label` nests under `parent` — a lead's id, or `null` for a root
   * — then returns it. The re-root assertion waits on `null` here: a nested worker becomes a root
   * only once its lead has genuinely left the registry.
   */
  async waitForParent(label: string, parent: number | null): Promise<TreeNode> {
    let node: TreeNode | undefined;
    try {
      await browser.waitUntil(
        async () => {
          node = (await this.nodes()).find((candidate) => candidate.label === label);
          return node !== undefined && node.parent === parent;
        },
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(
        `orchestration node "${label}" never reported parent ${parent ?? "null"}; last seen: ${
          node === undefined ? "no such node" : String(node.parent)
        }`,
      );
    }
    return node as TreeNode;
  },

  /** Waits until no node labelled `label` remains — the process left the registry. */
  async waitForGone(label: string): Promise<void> {
    try {
      await browser.waitUntil(
        async () => (await this.nodes()).every((node) => node.label !== label),
        { timeout: WAIT.core },
      );
    } catch {
      throw new Error(`orchestration node "${label}" never disappeared from the tree`);
    }
  },
};
