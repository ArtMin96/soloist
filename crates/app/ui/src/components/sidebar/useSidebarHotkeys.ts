import { useRef } from "react";
import {
  findSelectedTree,
  firstOfKind,
  firstProcessInTree,
  selectedKind,
} from "@/components/sidebar/sidebarNav";
import { bindingFromEvent, isEditableTarget, matchHotkey } from "@/lib/hotkeys";
import { projectCollapseKey, type ProjectTree } from "@/store/projects";
import { useHotkeys } from "@/store/hotkeysContext";

interface SidebarHotkeysState {
  trees: ProjectTree[];
  selectedId: number | null;
  setCollapsed: (key: string, value: boolean) => void;
  onSelect: (id: number) => void;
  onRestart: (id: number) => void;
}

// Returns a keydown handler to spread onto the sidebar nav element. Matches the pressed chord
// against sidebar-scope bindings from the live keymap and dispatches navigation, collapse, or
// restart actions. Fires for events that bubble from any child — process rows, group headers,
// buttons — so sidebar hotkeys are active whenever any part of the sidebar has focus.
export function useSidebarHotkeys(state: SidebarHotkeysState) {
  const { bindings } = useHotkeys();

  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  const stateRef = useRef(state);
  stateRef.current = state;

  function handleKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (isEditableTarget(event.target)) return;
    const pressed = bindingFromEvent(event.nativeEvent);
    if (!pressed) return;

    const action = matchHotkey(bindingsRef.current, "sidebar", pressed);
    if (!action) return;

    const { trees, selectedId, setCollapsed, onSelect, onRestart } = stateRef.current;
    const currentTree = findSelectedTree(trees, selectedId);

    switch (action) {
      case "restart_selection": {
        if (selectedId !== null) onRestart(selectedId);
        break;
      }
      case "next_project_group": {
        if (!currentTree) break;
        const idx = trees.findIndex((t) => t.project.id === currentTree.project.id);
        const first = trees[idx + 1] ? firstProcessInTree(trees[idx + 1]) : null;
        if (first) onSelect(first.id);
        break;
      }
      case "prev_project_group": {
        if (!currentTree) break;
        const idx = trees.findIndex((t) => t.project.id === currentTree.project.id);
        const first = trees[idx - 1] ? firstProcessInTree(trees[idx - 1]) : null;
        if (first) onSelect(first.id);
        break;
      }
      case "next_section": {
        if (!currentTree || selectedId === null) break;
        const kind = selectedKind(currentTree, selectedId);
        if (!kind) break;
        const kinds = currentTree.kinds.filter((k) => k.processes.length > 0);
        const next = kinds[kinds.findIndex((k) => k.kind === kind) + 1];
        if (next?.processes[0]) onSelect(next.processes[0].id);
        break;
      }
      case "prev_section": {
        if (!currentTree || selectedId === null) break;
        const kind = selectedKind(currentTree, selectedId);
        if (!kind) break;
        const kinds = currentTree.kinds.filter((k) => k.processes.length > 0);
        const prev = kinds[kinds.findIndex((k) => k.kind === kind) - 1];
        if (prev?.processes[0]) onSelect(prev.processes[0].id);
        break;
      }
      case "jump_to_agents": {
        const first = currentTree ? firstOfKind(currentTree, "Agent") : null;
        if (first) onSelect(first.id);
        break;
      }
      case "jump_to_commands": {
        const first = currentTree ? firstOfKind(currentTree, "Command") : null;
        if (first) onSelect(first.id);
        break;
      }
      case "jump_to_terminals": {
        const first = currentTree ? firstOfKind(currentTree, "Terminal") : null;
        if (first) onSelect(first.id);
        break;
      }
      case "collapse_or_section": {
        if (currentTree) setCollapsed(projectCollapseKey(currentTree.project.id), true);
        break;
      }
      case "expand_project": {
        if (currentTree) setCollapsed(projectCollapseKey(currentTree.project.id), false);
        break;
      }
      case "jump_to_parent_project": {
        // Jump to the first process in the current project — the nearest "top" of the tree.
        const first = currentTree ? firstProcessInTree(currentTree) : null;
        if (first) onSelect(first.id);
        break;
      }
      default:
        return;
    }

    event.preventDefault();
  }

  return handleKeyDown;
}
