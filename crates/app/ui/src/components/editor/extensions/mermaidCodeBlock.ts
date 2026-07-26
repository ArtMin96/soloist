import { ReactNodeViewRenderer } from "@tiptap/react";
import { CodeBlock } from "@tiptap/extension-code-block";
import { MermaidCodeBlockView } from "../MermaidCodeBlockView";

// The code-block node with a React NodeView bolted on: the schema, input rules, and Markdown
// round-trip of `@tiptap/extension-code-block` are kept exactly as-is (so ```lang still fences and
// serializes), and only the rendering is replaced. The NodeView decides per block whether to draw a
// diagram (mermaid) or a plain code block (everything else), so ordinary code is unaffected.
export const mermaidCodeBlock = CodeBlock.extend({
  addNodeView() {
    return ReactNodeViewRenderer(MermaidCodeBlockView);
  },
});
