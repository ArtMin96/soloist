import type { Editor, Range } from "@tiptap/react";
import { CodeBlock } from "@tiptap/extension-code-block";
import { MERMAID_LANGUAGE, MERMAID_STARTER_SOURCE } from "@/lib/mermaid";

// The single definition of what "insert a Mermaid diagram" does, so the slash menu and the toolbar
// land the exact same block and never drift. The starter is inserted as node JSON — a `codeBlock`
// carrying the mermaid language and a text child — rather than a string, so the newline in the starter
// survives (a string would be parsed as HTML and collapse it, breaking the diagram).
function mermaidBlockContent() {
  return {
    type: CodeBlock.name,
    attrs: { language: MERMAID_LANGUAGE },
    content: [{ type: "text", text: MERMAID_STARTER_SOURCE }],
  };
}

/**
 * Insert a starter Mermaid diagram block at the caret. When a `range` is given (the "/query" the slash
 * menu leaves behind), it is deleted first so no trigger text remains.
 */
export function insertMermaidDiagram(editor: Editor, range?: Range): void {
  const chain = editor.chain().focus();
  if (range) chain.deleteRange(range);
  chain.insertContent(mermaidBlockContent()).run();
}
