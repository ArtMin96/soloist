import {
  BanIcon,
  CircleCheckIcon,
  CircleDashedIcon,
  CircleDotIcon,
  type LucideIcon,
} from "lucide-react";
import type { CommentAuthor, TodoStatus } from "@/domain";
import type { Option } from "@/lib/appearance";

// The single source for a todo's declared lifecycle label. This is the status an agent *declares*
// (Open / In progress / Done / Blocked) — distinct from the derived blocker **gate** (`TodoView.blocked`),
// which is the one source of truth for whether it can complete. Kept monochrome on purpose: per
// DESIGN.md saturated color is spent only on process status, never invented for a second vocabulary.
export const TODO_STATUS: Record<TodoStatus, string> = {
  open: "Open",
  blocked: "Blocked",
  in_progress: "In progress",
  done: "Done",
};

// The order statuses are offered in — the natural workflow progression (Open → In progress →
// Blocked → Done), not the enum's declaration order. The one ordering both the editor's status
// select and the board's status facet render, so the sequence changes in exactly one place.
export const TODO_STATUS_ORDER: TodoStatus[] = ["open", "in_progress", "blocked", "done"];

// The display name for a comment's author, resolved once so every surface names it the same.
// A bound process shows its durable label; an external caller its label; an unbound caller's comment
// is "unattributed" — the core never forges one, so the UI never invents one either.
export function commentAuthorLabel(author: CommentAuthor | null): string {
  if (author == null) return "unattributed";
  return author.label;
}

/** How the board arranges its rows. Grouped is the default — most work comes out of a document. */
export type BoardView = "grouped" | "all";

// The one definition of the board's view options, so the toolbar's control and the board's own
// state agree on the same two values without a second literal list to drift.
export const BOARD_VIEWS: Option<BoardView>[] = [
  { value: "all", label: "All" },
  { value: "grouped", label: "By scratchpad" },
];

// The glyph each declared status wears, redundant with `TODO_STATUS`'s text label rather than a
// replacement for it — a row reads without color alone. Monochrome, same discipline as the label.
export const TODO_STATUS_ICON: Record<TodoStatus, LucideIcon> = {
  open: CircleDashedIcon,
  in_progress: CircleDotIcon,
  blocked: BanIcon,
  done: CircleCheckIcon,
};

// The exact wording for a row's unmet-blocker count — singular only at exactly one.
export function unmetBlockerLabel(count: number): string {
  return count === 1 ? "1 unmet blocker" : `${count} unmet blockers`;
}
