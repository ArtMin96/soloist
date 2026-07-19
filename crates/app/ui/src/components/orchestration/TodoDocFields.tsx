import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { humanizeName } from "@/lib/humanize";
import { TODO_STATUS, TODO_STATUS_ORDER } from "@/lib/todo";
import type { Option } from "@/lib/appearance";
import type { ScratchpadSummary, TodoStatus } from "@/domain";

/** The picker's "not derived from any scratchpad" choice — the default, and never an error. */
const NO_SCRATCHPAD = "None";

interface TodoDocFieldsProps {
  title: string;
  status: TodoStatus;
  /** The Markdown the body editor mounts with — read once (it is uncontrolled, remounted per doc). */
  initialBody: string;
  /** The project's scratchpads, offered as the documents this todo may derive from. */
  scratchpads: ScratchpadSummary[];
  /** The scratchpad's durable id, or null when the todo derives from none. */
  scratchpad: number | null;
  onTitleChange: (title: string) => void;
  onStatusChange: (status: TodoStatus) => void;
  onScratchpadChange: (scratchpad: number | null) => void;
  onBodyChange: (markdown: string) => void;
  /** Fired on Cmd/Ctrl+S inside the body — the caller flushes a pending save. */
  onSaveShortcut?: () => void;
  /** Fired when the body loses focus — the caller flushes a pending save. */
  onBlur?: () => void;
  /** The DOM id tying the visible title control to its accessible name. */
  titleId: string;
}

// The shared document fields a todo is authored with: a single-line title, the rich body editor, the
// status select, and the scratchpad the todo derives from. Presentational — the create form and the
// edit surface each own the field state and persistence, and both mount the one `components/editor`
// module here (no second editor), which is the reusability the board proves. Title, status, and the
// scratchpad are controlled by the parent; the body is uncontrolled and seeded once from
// `initialBody`.
//
// The scratchpad picker defaults to None and stays there until the author says otherwise: a todo
// that came from nowhere in particular is the ordinary case, so this is something to opt into, never
// something to opt out of.
export function TodoDocFields({
  title,
  status,
  initialBody,
  scratchpads,
  scratchpad,
  onTitleChange,
  onStatusChange,
  onScratchpadChange,
  onBodyChange,
  onSaveShortcut,
  onBlur,
  titleId,
}: TodoDocFieldsProps) {
  const options: Option<string | null>[] = [
    { value: null, label: NO_SCRATCHPAD },
    ...scratchpads.map((pad) => ({ value: String(pad.id), label: humanizeName(pad.name) })),
  ];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex items-center gap-2">
        <Input
          id={titleId}
          value={title}
          onChange={(event) => onTitleChange(event.target.value)}
          placeholder="Title"
          aria-label="Todo title"
          className="h-8 flex-1 font-[550]"
        />
        <Select value={status} onValueChange={(value) => onStatusChange(value as TodoStatus)}>
          <SelectTrigger size="sm" aria-label="Status" className="w-32 shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TODO_STATUS_ORDER.map((option) => (
              <SelectItem key={option} value={option}>
                {TODO_STATUS[option]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="flex items-center gap-2">
        {/* A plain caption, not a `label`: the underlying select is named by `aria-label`, whose
            text opens with the same word, so the visible and accessible names agree. */}
        <span className="shrink-0 text-[0.6875rem] font-[550] tracking-[0.01em] text-muted-foreground">
          Scratchpad
        </span>
        <NullableSelect<string>
          value={scratchpad === null ? null : String(scratchpad)}
          options={options}
          onValueChange={(value) => onScratchpadChange(value === null ? null : Number(value))}
          ariaLabel="Scratchpad this todo derives from"
          className="w-56"
        />
      </div>
      <div className="min-h-0 flex-1">
        <LazyRichTextEditor
          initialMarkdown={initialBody}
          ariaLabel="Todo body"
          placeholder="Add detail — press / for commands"
          onChange={onBodyChange}
          onSaveShortcut={onSaveShortcut}
          onBlur={onBlur}
        />
      </div>
    </div>
  );
}
