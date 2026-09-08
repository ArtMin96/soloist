import {
  DOCUMENT_ROLE,
  DOCUMENT_ROLE_ICON,
  DOCUMENT_ROLE_TONE,
  participantsLabel,
} from "@/lib/documentRole";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import type { DocumentItem } from "@/components/sidebar/documentItems";
import type { WorkSection } from "@/store/projects";

interface DocumentRowProps {
  item: DocumentItem;
  section: WorkSection;
}

// One coordination document in the sidebar's Todos or Scratchpads group: a status/file glyph, its
// title, and a trailing role glyph — collapsed to the icon alone so the fixed-width row keeps most
// of its space for the title, the way a status indicator collapses in the dense sidebar; the role
// word itself carries in the accessible name and the tooltip, which also names every participant. A
// plain `<button>` in a `<li>` rather than a tree row — it navigates to the document, it is never a
// keyboard-traversal stop the sidebar's arrow keys address. Hover and focus-visible match
// `ProcessRow`'s sidebar-row treatment rather than inventing a second one.
export function DocumentRow({ item, section }: DocumentRowProps) {
  const RoleIcon = DOCUMENT_ROLE_ICON[item.role];
  const roleLabel = DOCUMENT_ROLE[item.role];
  const multiple = item.participants.length > 1;
  const handleAttr =
    section === "todos"
      ? { "data-document-todo": item.handle }
      : { "data-document-scratchpad": item.handle };
  const ariaLabel = `${item.label}, ${roleLabel}${multiple ? `, ${item.participants.length} agents` : ""}`;

  return (
    <li>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            data-document-row
            data-document-role={item.role}
            {...handleAttr}
            aria-label={ariaLabel}
            onClick={item.activate}
            className="group/document-row flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-[0.8125rem] outline-none transition-colors duration-[var(--dur-select)] ease-out-quint hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring"
          >
            <item.icon
              aria-hidden
              className={cn("size-3.5 shrink-0", item.tone ?? "text-muted-foreground")}
            />
            <span className="min-w-0 flex-1 truncate">{item.label}</span>
            <span className="flex shrink-0 items-center gap-1 text-[0.6875rem] text-muted-foreground">
              <RoleIcon aria-hidden className={cn("size-3", DOCUMENT_ROLE_TONE[item.role])} />
              {multiple && (
                <span className="font-mono tabular-nums">×{item.participants.length}</span>
              )}
            </span>
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">{participantsLabel(item.participants)}</TooltipContent>
      </Tooltip>
    </li>
  );
}
