import { DocumentRow } from "@/components/sidebar/DocumentRow";
import { SidebarGroup } from "@/components/sidebar/SidebarGroup";
import type { DocumentItem } from "@/components/sidebar/documentItems";
import { WORK_SECTION_LABELS, type WorkSection } from "@/store/projects";

interface DocumentGroupProps {
  section: WorkSection;
  items: DocumentItem[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// One project's Todos or Scratchpads group: the same collapsible header a process-kind group
// wears, over a plain list of document rows. `role="list"`, never `role="tree"` — the sidebar's
// arrow-key traversal keeps addressing process rows only.
export function DocumentGroup({ section, items, open, onOpenChange }: DocumentGroupProps) {
  const label = WORK_SECTION_LABELS[section];
  return (
    <SidebarGroup label={label} count={items.length} open={open} onOpenChange={onOpenChange}>
      <ul
        role="list"
        data-document-section={section}
        aria-label={label}
        className="mt-0.5 flex flex-col gap-px pl-1"
      >
        {items.map((item) => (
          <DocumentRow key={item.key} item={item} section={section} />
        ))}
      </ul>
    </SidebarGroup>
  );
}
