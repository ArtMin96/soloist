import { FileTextIcon, type LucideIcon } from "lucide-react";
import { strongestRole } from "@/lib/documentRole";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS_ICON, TODO_STATUS_TONE } from "@/lib/todo";
import type { WorkSection } from "@/store/projects";
import type { DocumentParticipant, DocumentRole, ProjectWork } from "@/domain";

/** One document row's normalized shape, so `DocumentRow` renders without knowing todo from
 * scratchpad. */
export interface DocumentItem {
  /** React key — stable across re-reads. */
  key: string;
  /** What the row reads as: a todo's title, a scratchpad's humanized name. */
  label: string;
  /** The leading glyph: a todo's status icon, a scratchpad's file icon. */
  icon: LucideIcon;
  /** The leading glyph's tone class; null for a scratchpad, which has no status. */
  tone: string | null;
  role: DocumentRole;
  participants: DocumentParticipant[];
  /** The value the row's identity attribute carries: the durable id the core gave the document. */
  handle: number;
  activate: () => void;
}

// Normalizes a project's live work into the rows one document group renders — the join between
// the core's todo/scratchpad shapes and what a sidebar row needs, so `DocumentRow` stays generic
// over which kind of document it shows. Pure: no work yet loaded (or a filter hiding it) yields no
// rows, and the core's own order is kept rather than re-sorted here.
export function documentItems(
  work: ProjectWork | undefined,
  section: WorkSection,
  onOpenTodo: (todo: number) => void,
  onOpenScratchpad: (scratchpad: number) => void,
): DocumentItem[] {
  if (!work) return [];
  if (section === "todos") {
    return work.todos.map((todo) => ({
      key: `todo-${todo.id}`,
      label: todo.title,
      icon: TODO_STATUS_ICON[todo.status],
      tone: TODO_STATUS_TONE[todo.status],
      role: strongestRole(todo.participants),
      participants: todo.participants,
      handle: todo.id,
      activate: () => onOpenTodo(todo.id),
    }));
  }
  return work.scratchpads.map((pad) => ({
    key: `scratchpad-${pad.id}`,
    label: humanizeName(pad.name),
    icon: FileTextIcon,
    tone: null,
    role: strongestRole(pad.participants),
    participants: pad.participants,
    handle: pad.id,
    activate: () => onOpenScratchpad(pad.id),
  }));
}
