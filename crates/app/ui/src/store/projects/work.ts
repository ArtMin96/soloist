/** The two document groups a project node shows under its process groups. */
export type WorkSection = "todos" | "scratchpads";

/** The fixed order the two groups render in, and the heading each wears. */
export const WORK_SECTIONS: WorkSection[] = ["todos", "scratchpads"];
export const WORK_SECTION_LABELS: Record<WorkSection, string> = {
  todos: "Todos",
  scratchpads: "Scratchpads",
};

/** Stable collapse-state key for one project's document group. */
export function workCollapseKey(id: number, section: WorkSection): string {
  return `work:${id}:${section}`;
}
