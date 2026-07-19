import { Extension } from "@tiptap/react";
import { searchPlugin } from "./searchPlugin";

// Registers the in-note find plugin so every editor surface carries it. The plugin stays idle until a
// query arrives, so its presence costs nothing until the user opens the find bar.
export const searchExtension = Extension.create({
  name: "editorSearch",
  addProseMirrorPlugins() {
    return [searchPlugin];
  },
});
