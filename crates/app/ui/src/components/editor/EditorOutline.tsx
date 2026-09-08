import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";
import { useEditorState, type Editor } from "@tiptap/react";
import { cn } from "@/lib/utils";
import { useScrollSpy } from "@/store/useScrollSpy";

// One heading in the note, with the document position its row places the caret at.
interface OutlineEntry {
  level: number;
  text: string;
  pos: number;
}

// The threshold below which an outline is noise: one or zero headings do not need a rail.
const MIN_HEADINGS = 2;

// The rendered headings inside the editor body, read in document order — the same sequence
// `collectHeadings` reads off the model, so index N of one addresses index N of the other.
const HEADING_SELECTOR = "h1, h2, h3, h4, h5, h6";

// The editor's own scroll container (its styling lives in editor.css), which the rail tracks.
const SCROLL_CONTAINER_SELECTOR = ".tiptap-scroll";

// Horizontal step per heading level, so the rail reads as the note's shape at a glance.
const LEVEL_INDENT_PX = 10;

const UNTITLED = "Untitled heading";

function collectHeadings(editor: Editor): OutlineEntry[] {
  const entries: OutlineEntry[] = [];
  editor.state.doc.descendants((node, pos) => {
    if (node.type.name === "heading") {
      entries.push({ level: node.attrs.level as number, text: node.textContent, pos });
    }
  });
  return entries;
}

function sameElements(a: HTMLElement[], b: HTMLElement[]): boolean {
  return a.length === b.length && a.every((el, i) => el === b[i]);
}

// A slim heading rail beside the editor: the note's table of contents. It renders only once the note
// has at least a couple of headings, so short notes stay uncluttered. The heading list comes from the
// TipTap model reactively via `useEditorState`, comparing a cheap signature so the rail re-renders
// only when the outline itself changes.
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
  return <OutlineRail editor={editor} headings={headings} />;
}

/** What the rail reads off the DOM ProseMirror renders: the scroll container and the heading nodes. */
interface RenderedHeadings {
  container: HTMLElement | null;
  targets: HTMLElement[];
}

const NO_RENDERED_HEADINGS: RenderedHeadings = { container: null, targets: [] };

// Reads the headings ProseMirror has actually painted into `editor.view.dom` — its own DOM subtree,
// written outside React's render cycle, so the model's `headingCount` alone cannot say when the DOM
// has caught up with it. `useSyncExternalStore` is what makes this reactive to that: the "update"
// event is ProseMirror's own signal that its DOM just changed, and the cached snapshot keeps
// `targets`/`container` referentially stable across reads that found nothing new, so the rail does
// not re-render itself over a change elsewhere in the note.
function useRenderedHeadings(editor: Editor, headingCount: number): RenderedHeadings {
  const cacheRef = useRef(NO_RENDERED_HEADINGS);

  const subscribe = useCallback(
    (onStoreChange: () => void) => {
      editor.on("update", onStoreChange);
      return () => void editor.off("update", onStoreChange);
    },
    [editor],
  );

  const getSnapshot = useCallback((): RenderedHeadings => {
    const root = editor.view.dom;
    const container = root.closest<HTMLElement>(SCROLL_CONTAINER_SELECTOR);
    // Trimming to the model's count keeps the two indexed by the same range even mid-commit.
    const targets = Array.from(root.querySelectorAll<HTMLElement>(HEADING_SELECTOR)).slice(
      0,
      headingCount,
    );
    const cached = cacheRef.current;
    if (cached.container !== container || !sameElements(cached.targets, targets)) {
      cacheRef.current = { container, targets };
    }
    return cacheRef.current;
  }, [editor, headingCount]);

  return useSyncExternalStore(subscribe, getSnapshot);
}

// The rail proper, mounted only when there is an outline to show — so a note without one never pays
// for the observers that track the reading position.
function OutlineRail({ editor, headings }: { editor: Editor; headings: OutlineEntry[] }) {
  const { container, targets } = useRenderedHeadings(editor, headings.length);
  const activeRowRef = useRef<HTMLLIElement>(null);

  const { activeIndex, scrollToTarget, remeasure } = useScrollSpy(container, targets);

  // Editing above a heading moves it without changing the outline, so cached positions go stale on
  // any document change — not only one that adds or removes a heading.
  useEffect(() => {
    editor.on("update", remeasure);
    return () => void editor.off("update", remeasure);
  }, [editor, remeasure]);

  // Keep the reading position visible in the rail itself, so a long outline follows the document.
  useEffect(() => {
    activeRowRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIndex]);

  return (
    // A selection scope of its own: the active heading reads azure only while the keyboard is in
    // the rail, and neutral while you are reading or typing in the document beside it — so the
    // outline never competes with the editor for the one emphasized selection on screen.
    <nav
      aria-label="Outline"
      data-selection-scope
      className="w-40 shrink-0 overflow-y-auto border-l py-2 pr-1 pl-2"
    >
      <ul className="flex flex-col gap-px">
        {headings.map((heading, index) => {
          const active = index === activeIndex;
          const label = heading.text || UNTITLED;
          return (
            <li key={heading.pos} ref={active ? activeRowRef : null}>
              <button
                type="button"
                aria-current={active ? "true" : undefined}
                onClick={() => {
                  // Place the caret first so typing after the jump lands in the section, then own
                  // the scroll — the caret move scrolls too, and the later call wins.
                  editor
                    .chain()
                    .focus()
                    .setTextSelection(heading.pos + 1)
                    .run();
                  scrollToTarget(index);
                }}
                style={{ paddingInlineStart: `${(heading.level - 1) * LEVEL_INDENT_PX}px` }}
                className={cn(
                  "w-full truncate rounded-md py-0.5 pr-1.5 text-left text-[0.6875rem] leading-tight",
                  "transition-colors duration-[var(--dur-select)] ease-out-quint",
                  "focus-visible:bg-muted focus-visible:text-foreground focus-visible:outline-none",
                  active
                    ? "bg-[var(--sel-fill)] text-foreground hover:bg-[var(--sel-fill-hover)]"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )}
                title={label}
              >
                {label}
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
