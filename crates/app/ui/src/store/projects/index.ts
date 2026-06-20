// The projects domain on the frontend: the single place project behaviour lives. The store
// (read-model + the open action), the project↔process tree projection with its visibility and
// count rules, and the per-project view helpers (monogram, collapse keys). Consumers — the
// sidebar and App — import from here and only render; they do not re-implement how projects
// are grouped, named, shown, or which projects appear.
export { useProjects, type ProjectStore } from "@/store/projects/useProjects";
export {
  groupByProject,
  runningCount,
  type ProjectTree,
  type RunningCount,
} from "@/store/projects/tree";
export { monogram, projectCollapseKey, kindCollapseKey } from "@/store/projects/view";
