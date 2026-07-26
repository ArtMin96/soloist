import { useCallback, useEffect, useRef, useState } from "react";
import { NodeViewContent, NodeViewWrapper, type NodeViewProps } from "@tiptap/react";
import { Check, Copy } from "lucide-react";
import { SegmentedControl } from "@/components/SegmentedControl";
import { MermaidDiagram } from "@/components/mermaid/MermaidDiagram";
import { MERMAID_LANGUAGE } from "@/lib/mermaid";
import type { Option } from "@/lib/appearance";
import "./mermaidBlock.css";

type MermaidViewMode = "preview" | "source";

const MODE_OPTIONS: Option<MermaidViewMode>[] = [
  { value: "preview", label: "Preview" },
  { value: "source", label: "Source" },
];

// How long the copy button shows its confirmation before returning to the copy icon.
const COPIED_RESET_MS = 1200;

/**
 * The code-block NodeView. A block tagged with the mermaid language renders as a live diagram with a
 * Preview/Source toggle; every other code block renders as an ordinary, untouched `<pre><code>`.
 *
 * In preview the editable content DOM stays mounted (ProseMirror requires it) but hidden, so the text
 * still syncs and the block round-trips; clicking the diagram drops into source mode and places the
 * caret. A source that fails to parse pulls the block into source mode on its own — unless the reader
 * has explicitly chosen a view, in which case their choice stands and the error banner shows instead.
 */
export function MermaidCodeBlockView({ node, editor, getPos }: NodeViewProps) {
  const isMermaid = node.attrs.language === MERMAID_LANGUAGE;

  const [mode, setMode] = useState<MermaidViewMode>("preview");
  const [copied, setCopied] = useState(false);
  // The reader has taken control of which view shows, so a parse error must not override it.
  const userPinnedRef = useRef(false);
  // A click-to-edit is pending; focus the source once it is actually visible (not while hidden).
  const focusPendingRef = useRef(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), COPIED_RESET_MS);
    return () => window.clearTimeout(timer);
  }, [copied]);

  useEffect(() => {
    if (mode !== "source" || !focusPendingRef.current) return;
    focusPendingRef.current = false;
    const pos = getPos();
    if (typeof pos === "number") editor.commands.focus(pos + 1);
  }, [mode, editor, getPos]);

  const chooseMode = useCallback((next: MermaidViewMode) => {
    userPinnedRef.current = true;
    setMode(next);
  }, []);

  const editSource = useCallback(() => {
    userPinnedRef.current = true;
    focusPendingRef.current = true;
    setMode("source");
  }, []);

  const handleParse = useCallback((ok: boolean) => {
    if (!ok && !userPinnedRef.current) setMode("source");
  }, []);

  const copySource = useCallback(() => {
    void navigator.clipboard?.writeText(node.textContent);
    setCopied(true);
  }, [node]);

  if (!isMermaid) {
    return (
      <NodeViewWrapper as="pre">
        <NodeViewContent<"code"> as="code" />
      </NodeViewWrapper>
    );
  }

  const preview = mode === "preview";
  return (
    <NodeViewWrapper className="mermaid-block">
      <div className="mermaid-block-header" contentEditable={false}>
        <span className="mermaid-block-label">Mermaid</span>
        <div className="mermaid-block-actions">
          <SegmentedControl
            value={mode}
            options={MODE_OPTIONS}
            onChange={chooseMode}
            ariaLabel="Diagram view"
          />
          <button
            type="button"
            className="mermaid-block-copy"
            onClick={copySource}
            aria-label={copied ? "Copied" : "Copy diagram source"}
            title="Copy source"
          >
            {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
          </button>
        </div>
      </div>
      {preview && (
        <div
          className="mermaid-block-preview"
          contentEditable={false}
          role="button"
          tabIndex={0}
          title="Click to edit source"
          onClick={editSource}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              editSource();
            }
          }}
        >
          <MermaidDiagram source={node.textContent} onParse={handleParse} />
        </div>
      )}
      {/* The content DOM is always present so ProseMirror can sync the source; preview just hides it. */}
      <pre
        className={
          preview ? "mermaid-block-source mermaid-block-source--hidden" : "mermaid-block-source"
        }
      >
        <NodeViewContent<"code"> as="code" />
      </pre>
    </NodeViewWrapper>
  );
}
