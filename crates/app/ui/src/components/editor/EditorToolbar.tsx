import type { ComponentType } from "react";
import { useEditorState, type Editor } from "@tiptap/react";
import {
  Bold,
  Code,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  List,
  ListOrdered,
  ListTodo,
  SquareCode,
  Strikethrough,
  TextQuote,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// One toolbar control: the icon, its accessible name, whether it reads active in the current
// selection, and the toggle it runs. Keeping them as data means the row is one map, not twelve
// near-identical buttons.
interface ToolAction {
  icon: ComponentType<{ className?: string }>;
  label: string;
  active: (editor: Editor) => boolean;
  run: (editor: Editor) => void;
}

// A visual break between related groups, rendered as a hairline.
type ToolEntry = ToolAction | "separator";

const TOOLS: ToolEntry[] = [
  {
    icon: Bold,
    label: "Bold",
    active: (e) => e.isActive("bold"),
    run: (e) => e.chain().focus().toggleBold().run(),
  },
  {
    icon: Italic,
    label: "Italic",
    active: (e) => e.isActive("italic"),
    run: (e) => e.chain().focus().toggleItalic().run(),
  },
  {
    icon: Strikethrough,
    label: "Strikethrough",
    active: (e) => e.isActive("strike"),
    run: (e) => e.chain().focus().toggleStrike().run(),
  },
  {
    icon: Code,
    label: "Inline code",
    active: (e) => e.isActive("code"),
    run: (e) => e.chain().focus().toggleCode().run(),
  },
  "separator",
  {
    icon: Heading1,
    label: "Heading 1",
    active: (e) => e.isActive("heading", { level: 1 }),
    run: (e) => e.chain().focus().toggleHeading({ level: 1 }).run(),
  },
  {
    icon: Heading2,
    label: "Heading 2",
    active: (e) => e.isActive("heading", { level: 2 }),
    run: (e) => e.chain().focus().toggleHeading({ level: 2 }).run(),
  },
  {
    icon: Heading3,
    label: "Heading 3",
    active: (e) => e.isActive("heading", { level: 3 }),
    run: (e) => e.chain().focus().toggleHeading({ level: 3 }).run(),
  },
  "separator",
  {
    icon: List,
    label: "Bullet list",
    active: (e) => e.isActive("bulletList"),
    run: (e) => e.chain().focus().toggleBulletList().run(),
  },
  {
    icon: ListOrdered,
    label: "Numbered list",
    active: (e) => e.isActive("orderedList"),
    run: (e) => e.chain().focus().toggleOrderedList().run(),
  },
  {
    icon: ListTodo,
    label: "To-do list",
    active: (e) => e.isActive("taskList"),
    run: (e) => e.chain().focus().toggleTaskList().run(),
  },
  "separator",
  {
    icon: TextQuote,
    label: "Quote",
    active: (e) => e.isActive("blockquote"),
    run: (e) => e.chain().focus().toggleBlockquote().run(),
  },
  {
    icon: SquareCode,
    label: "Code block",
    active: (e) => e.isActive("codeBlock"),
    run: (e) => e.chain().focus().toggleCodeBlock().run(),
  },
];

const ACTIONS = TOOLS.filter((entry): entry is ToolAction => entry !== "separator");

// The formatting toolbar for a rich-text editor. Each control is a ghost icon toggle that reflects
// whether the mark/block is active in the current selection (`aria-pressed`) — the same ghost-button
// vocabulary the process rows use, so nothing new is introduced. Active state re-reads through
// `useEditorState` on every selection change without re-rendering the whole editor.
export function EditorToolbar({ editor }: { editor: Editor }) {
  const active = useEditorState({
    editor,
    selector: ({ editor }) => ACTIONS.map((action) => action.active(editor)),
    equalityFn: (a, b) => b !== null && a.length === b.length && a.every((v, i) => v === b[i]),
  });

  let actionIndex = 0;
  return (
    <div role="toolbar" aria-label="Formatting" className="flex items-center gap-0.5">
      {TOOLS.map((entry, index) => {
        if (entry === "separator") {
          return <div key={`sep-${index}`} className="mx-1 h-4 w-px bg-border/70" aria-hidden />;
        }
        const isActive = active[actionIndex++];
        const Icon = entry.icon;
        return (
          <Button
            key={entry.label}
            type="button"
            variant="ghost"
            size="icon-sm"
            aria-label={entry.label}
            aria-pressed={isActive}
            title={entry.label}
            // Keep the caret in the document: the toolbar is a control, not a focus target.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => entry.run(editor)}
            className={cn("size-7", isActive && "bg-muted text-foreground")}
          >
            <Icon className="size-3.5" />
          </Button>
        );
      })}
    </div>
  );
}
