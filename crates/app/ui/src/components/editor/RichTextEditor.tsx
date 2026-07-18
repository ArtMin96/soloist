import { useEffect, useRef } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import { buildEditorExtensions } from "./editorExtensions";
import { EditorOutline } from "./EditorOutline";
import { EditorToolbar } from "./EditorToolbar";
import "./editor.css";

export interface RichTextEditorProps {
  /**
   * The Markdown the editor mounts with. The editor is uncontrolled thereafter — a new document
   * arrives by remounting with a fresh React `key`, never by pushing `initialMarkdown` back in, so
   * the per-document undo history is preserved and a save never echoes into the caret.
   */
  initialMarkdown: string;
  /** Fired with the editor's Markdown on every edit. Debouncing and persistence are the caller's. */
  onChange: (markdown: string) => void;
  /** Fired when the user presses Cmd/Ctrl+S inside the editor — the caller flushes any pending save. */
  onSaveShortcut?: () => void;
  /** Fired when the editor loses focus — the caller flushes any pending save. */
  onBlur?: () => void;
  editable?: boolean;
  /** The empty-state prompt shown in the first empty block. */
  placeholder?: string;
  /** Render the formatting toolbar above the content. */
  toolbar?: boolean;
  /** Render the heading outline rail (it shows only once the note has a couple of headings). */
  outline?: boolean;
  /** Enable the "/" command menu. */
  slash?: boolean;
  /** The accessible name for the editable region — also the stable handle tests anchor on. */
  ariaLabel?: string;
}

// The reusable rich-text editor: an uncontrolled TipTap surface that speaks Markdown in and out.
// It confines every @tiptap import to this module (it is loaded lazily, so its dependencies land in
// their own chunk and never touch the initial bundle) and holds no feature knowledge — scratchpads,
// todos, and templates all mount it and pass their own persistence through the callbacks.
export default function RichTextEditor({
  initialMarkdown,
  onChange,
  onSaveShortcut,
  onBlur,
  editable = true,
  placeholder = "Press / for commands",
  toolbar = true,
  outline = false,
  slash = true,
  ariaLabel,
}: RichTextEditorProps) {
  // Read the current callbacks through refs so changing them never re-creates the editor (which
  // would drop the undo history and caret).
  const onChangeRef = useRef(onChange);
  const onSaveRef = useRef(onSaveShortcut);
  const onBlurRef = useRef(onBlur);
  onChangeRef.current = onChange;
  onSaveRef.current = onSaveShortcut;
  onBlurRef.current = onBlur;

  const editor = useEditor({
    editable,
    extensions: buildEditorExtensions({ placeholder, slash }),
    content: "",
    // Create the editor from an effect, never during render: the module is loaded lazily behind a
    // Suspense boundary, and React can render-then-discard the first pass on resume. Building the
    // editor (a side effect) inline would then leave a destroyed instance behind a live callback.
    immediatelyRender: false,
    editorProps: {
      attributes: {
        class: "tiptap-body",
        role: "textbox",
        "aria-multiline": "true",
        "data-editor": "rich-text",
        ...(ariaLabel ? { "aria-label": ariaLabel } : {}),
      },
      handleKeyDown: (_view, event) => {
        if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
          event.preventDefault();
          onSaveRef.current?.();
          return true;
        }
        return false;
      },
    },
    onUpdate: ({ editor }) => onChangeRef.current(editor.getMarkdown()),
    onBlur: () => onBlurRef.current?.(),
  });

  // Seed the initial Markdown once the editor exists, without emitting an update — so `onChange` does
  // not fire on load and the empty starting doc never counts as an edit.
  useEffect(() => {
    if (!editor) return;
    editor.commands.setContent(initialMarkdown, { contentType: "markdown", emitUpdate: false });
    // The editor is recreated per document (a fresh key), so seeding once on creation is correct;
    // re-running on `initialMarkdown` would clobber live edits.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editor]);

  return (
    <div className="tiptap-shell">
      {toolbar && editor && (
        <div className="tiptap-toolbar">
          <EditorToolbar editor={editor} />
        </div>
      )}
      <div className="tiptap-main">
        <EditorContent editor={editor} className="tiptap-scroll" />
        {outline && editor && <EditorOutline editor={editor} />}
      </div>
    </div>
  );
}
