import { useEffect, useRef, useState } from "react";
import type { Editor } from "@tiptap/react";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getSearchStatus, setSearchQuery, stepSearch } from "./search/searchPlugin";

interface EditorFindBarProps {
  editor: Editor;
  onClose: () => void;
}

// A compact find bar overlaying the top-right of the note. It drives the always-present search
// plugin: typing sets the query live, Enter / Shift+Enter step matches with wrap-around, and Esc
// closes and hands focus back to the note. It keeps no match state of its own — the plugin owns the
// matches, and this mirrors {total, currentIndex} back on every transaction so the counter stays live
// even while the note is edited behind the bar.
export function EditorFindBar({ editor, onClose }: EditorFindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState({ total: 0, currentIndex: 0 });

  // Seed from the current selection so "find what I highlighted" is one keystroke, then focus and
  // select the input. Runs once on open; later edits arrive through the transaction subscription.
  useEffect(() => {
    const { from, to } = editor.state.selection;
    const selected = from < to ? editor.state.doc.textBetween(from, to, " ") : "";
    const seed = selected.includes("\n") ? "" : selected.trim();
    if (seed) {
      setQuery(seed);
      setSearchQuery(editor.view, seed);
    }
    setStatus(getSearchStatus(editor.state));
    inputRef.current?.focus();
    inputRef.current?.select();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // The plugin recomputes on every doc edit and on every query/step; mirror its status from one place.
  useEffect(() => {
    const sync = () => setStatus(getSearchStatus(editor.state));
    editor.on("transaction", sync);
    return () => {
      editor.off("transaction", sync);
    };
  }, [editor]);

  function runQuery(next: string) {
    setQuery(next);
    setSearchQuery(editor.view, next);
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "Enter") {
      event.preventDefault();
      stepSearch(editor.view, event.shiftKey ? -1 : 1);
    }
  }

  const total = status.total;
  const currentDisplay = total === 0 ? 0 : status.currentIndex + 1;

  return (
    <search className="tiptap-find" aria-label="Find in note">
      <Search className="tiptap-find-icon" aria-hidden />
      <Input
        ref={inputRef}
        value={query}
        onChange={(event) => runQuery(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Find"
        aria-label="Find in note"
        className="tiptap-find-input"
      />
      <span className="tiptap-find-count" aria-live="polite">
        {currentDisplay} / {total}
      </span>
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={() => stepSearch(editor.view, -1)}
        disabled={total === 0}
        aria-label="Previous match"
      >
        <ChevronUp />
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        onClick={() => stepSearch(editor.view, 1)}
        disabled={total === 0}
        aria-label="Next match"
      >
        <ChevronDown />
      </Button>
      <Button variant="ghost" size="icon-xs" onClick={onClose} aria-label="Close find">
        <X />
      </Button>
    </search>
  );
}
