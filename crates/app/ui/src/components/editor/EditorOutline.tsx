import { useEditorState, type Editor } from "@tiptap/react";
import { cn } from "@/lib/utils";

// One heading in the note, with the document position its row jumps the caret to.
interface OutlineEntry {
  level: number;
  text: string;
  pos: number;
}

// The threshold below which an outline is noise: one or zero headings do not need a rail.
const MIN_HEADINGS = 2;

function collectHeadings(editor: Editor): OutlineEntry[] {
  const entries: OutlineEntry[] = [];
  editor.state.doc.descendants((node, pos) => {
    if (node.type.name === "heading") {
      entries.push({ level: node.attrs.level as number, text: node.textContent, pos });
    }
  });
  return entries;
}

// A slim heading rail beside the editor: the note's headings as clickable rows, indented by level,
// that scroll the body to the heading and place the caret there. It renders only once the note has at
// least a couple of headings, so short notes stay uncluttered. Reads the heading list reactively via
// `useEditorState`, comparing a cheap string signature so it re-renders only when the outline changes.
export function EditorOutline({ editor }: { editor: Editor }) {
  const headings = useEditorState({
    editor,
    selector: ({ editor }) => collectHeadings(editor),
    equalityFn: (a, b) =>
      b !== null &&
      a.length === b.length &&
      a.every((h, i) => h.pos === b[i].pos && h.level === b[i].level && h.text === b[i].text),
  });

  if (headings.length < MIN_HEADINGS) return null;

  return (
    <nav aria-label="Outline" className="w-40 shrink-0 overflow-y-auto border-l py-2 pr-1 pl-2">
      <ul className="flex flex-col gap-px">
        {headings.map((heading) => (
          <li key={heading.pos}>
            <button
              type="button"
              onClick={() =>
                editor
                  .chain()
                  .focus()
                  .setTextSelection(heading.pos + 1)
                  .scrollIntoView()
                  .run()
              }
              style={{ paddingInlineStart: `${(heading.level - 1) * 10}px` }}
              className={cn(
                "w-full truncate rounded-md py-0.5 pr-1.5 text-left text-[0.6875rem] leading-tight text-muted-foreground",
                "transition-colors duration-[var(--dur-fast)] ease-out-quint",
                "hover:bg-muted hover:text-foreground focus-visible:bg-muted focus-visible:text-foreground focus-visible:outline-none",
              )}
              title={heading.text || "Untitled heading"}
            >
              {heading.text || "Untitled heading"}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
