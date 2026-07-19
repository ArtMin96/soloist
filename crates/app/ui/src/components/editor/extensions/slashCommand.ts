import { Extension, ReactRenderer } from "@tiptap/react";
import { Suggestion } from "@tiptap/suggestion";
import { SlashCommandList, type SlashCommandListHandle } from "../SlashCommandList";
import { filterSlashItems, type SlashItem } from "../slashItems";

// The "/" slash-command extension: a thin wrapper over @tiptap/suggestion that opens a React menu of
// block structures. The menu is mounted and kept anchored to the caret by the plugin's managed
// positioning (Floating UI under the hood); we only build the React view and tear it down. The item
// definitions and their matching live in ../slashItems, so this file holds no command logic.
export const slashCommand = Extension.create({
  name: "slashCommand",

  addProseMirrorPlugins() {
    return [
      Suggestion<SlashItem>({
        editor: this.editor,
        char: "/",
        allowSpaces: false,
        // The menu inserts the block itself (after deleting the "/query"); this is the entry point
        // the list calls with the chosen item.
        command: ({ editor, range, props }) => props.run(editor, range),
        items: ({ query }) => filterSlashItems(query),
        render: () => {
          let component: ReactRenderer<SlashCommandListHandle> | null = null;
          let unmount: (() => void) | null = null;

          return {
            onStart: (props) => {
              component = new ReactRenderer(SlashCommandList, {
                props: { items: props.items, command: props.command, query: props.query },
                editor: props.editor,
              });
              // Managed positioning: the plugin appends the element, anchors it to the caret rect,
              // and repositions on scroll/resize. It returns the unmount to call from onExit.
              unmount = props.mount?.(component.element) ?? null;
            },
            onUpdate: (props) => {
              component?.updateProps({
                items: props.items,
                command: props.command,
                query: props.query,
              });
            },
            // The list consumes arrow/Enter; Escape is handled by the plugin, which then calls onExit.
            onKeyDown: (props) => component?.ref?.onKeyDown(props) ?? false,
            onExit: () => {
              unmount?.();
              component?.destroy();
              component = null;
              unmount = null;
            },
          };
        },
      }),
    ];
  },
});
