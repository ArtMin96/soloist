import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { LazyRichTextEditor } from "@/components/editor/LazyRichTextEditor";
import { TODO_STATUS, TODO_STATUS_ORDER } from "@/lib/todo";
import type { TodoStatus } from "@/domain";

interface TodoDocFieldsProps {
  title: string;
  status: TodoStatus;
  /** The Markdown the body editor mounts with — read once (it is uncontrolled, remounted per doc). */
  initialBody: string;
  onTitleChange: (title: string) => void;
  onStatusChange: (status: TodoStatus) => void;
  onBodyChange: (markdown: string) => void;
  /** Fired on Cmd/Ctrl+S inside the body — the caller flushes a pending save. */
  onSaveShortcut?: () => void;
  /** Fired when the body loses focus — the caller flushes a pending save. */
  onBlur?: () => void;
  /** The DOM id tying the visible title control to its accessible name. */
  titleId: string;
}

// The shared document fields a todo is authored with: a single-line title, the rich body editor, and
// the status select. Presentational — the create form and the edit surface each own the field state
// and persistence, and both mount the one `components/editor` module here (no second editor), which
// is the reusability the board proves. Title and status are controlled by the parent; the body is
// uncontrolled and seeded once from `initialBody`.
export function TodoDocFields({
  title,
  status,
  initialBody,
  onTitleChange,
  onStatusChange,
  onBodyChange,
  onSaveShortcut,
  onBlur,
  titleId,
}: TodoDocFieldsProps) {
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
