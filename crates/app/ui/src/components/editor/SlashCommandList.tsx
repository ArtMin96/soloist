import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import type { SlashItem } from "./slashItems";

// The imperative surface the suggestion plugin drives: it forwards keydowns here so arrow/Enter
// navigate the menu while the caret stays in the editor. Returning true means "handled" — the plugin
// then swallows the key.
export interface SlashCommandListHandle {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
}

interface SlashCommandListProps {
  items: SlashItem[];
  /** Runs the chosen item (deletes the "/query" and inserts the block); wired by the plugin. */
  command: (item: SlashItem) => void;
  /** The text typed after "/", which identifies one filtered menu. */
  query: string;
}

// The "/" command palette, mounted by the suggestion plugin via a ReactRenderer and positioned by
// Floating UI. A keyboard-first single-select listbox: arrows move the highlight (wrapping), Enter
// runs the highlighted item, the mouse can hover to highlight and click to run. Escape is handled by
// the plugin itself, which closes the menu and calls the render's cleanup.
const SlashCommandMenu = forwardRef<SlashCommandListHandle, Omit<SlashCommandListProps, "query">>(
  function SlashCommandMenu({ items, command }, ref) {
    const [active, setActive] = useState(0);
    const activeRef = useRef<HTMLButtonElement>(null);

    // Keep the highlighted row in view as the arrows walk a list longer than the popup.
    useEffect(() => {
      activeRef.current?.scrollIntoView({ block: "nearest" });
    }, [active]);

    useImperativeHandle(
      ref,
      () => ({
        onKeyDown: ({ event }) => {
          if (items.length === 0) return false;
          if (event.key === "ArrowDown") {
            setActive((current) => (current + 1) % items.length);
            return true;
          }
          if (event.key === "ArrowUp") {
            setActive((current) => (current - 1 + items.length) % items.length);
            return true;
          }
          if (event.key === "Enter") {
            const item = items[active];
            if (item) command(item);
            return true;
          }
          return false;
        },
      }),
      [items, active, command],
    );

    if (items.length === 0) {
      return <div className="slash-menu slash-menu--empty">No matching blocks</div>;
    }

    return (
      <div role="listbox" aria-label="Insert block" className="slash-menu">
        {items.map((item, index) => (
          <button
            key={item.title}
            ref={index === active ? activeRef : undefined}
            type="button"
            role="option"
            aria-selected={index === active}
            className={cn("slash-item", index === active && "slash-item--active")}
            onMouseEnter={() => setActive(index)}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => command(item)}
          >
            <span className="slash-item-title">{item.title}</span>
            <span className="slash-item-hint">{item.hint}</span>
          </button>
        ))}
      </div>
    );
  },
);

// Gives each query its own menu instance. A re-filter must drop the highlight back to the first row
// so it never points past the shorter list, and remounting on the query does that in the same commit
// that delivers the new items — where clearing the highlight from an effect would paint one frame of
// the old index against the new list, and briefly pair an Enter with the wrong item.
export const SlashCommandList = forwardRef<SlashCommandListHandle, SlashCommandListProps>(
  function SlashCommandList({ items, command, query }, ref) {
    return <SlashCommandMenu key={query} ref={ref} items={items} command={command} />;
  },
);
