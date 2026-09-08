import { EyeIcon, HammerIcon, PencilIcon, type LucideIcon } from "lucide-react";
import type { DocumentParticipant, DocumentRole } from "@/domain";

// The single source for a document participant's declared engagement label — the sidebar's Todos
// and Scratchpads rows say this beside every document.
export const DOCUMENT_ROLE: Record<DocumentRole, string> = {
  implementing: "Implementing",
  editing: "Editing",
  reading: "Reading",
};

// The glyph each role wears, redundant with `DOCUMENT_ROLE`'s text label rather than a
// replacement for it — a role reads without color alone.
export const DOCUMENT_ROLE_ICON: Record<DocumentRole, LucideIcon> = {
  implementing: HammerIcon,
  editing: PencilIcon,
  reading: EyeIcon,
};

// The hue each role carries, reusing the process-status tokens the same way `lib/todo.ts` does
// rather than minting a second palette — they are already theme-derived, and a theme that
// restyles one restyles both.
export const DOCUMENT_ROLE_TONE: Record<DocumentRole, string> = {
  implementing: "text-status-running",
  editing: "text-status-attention",
  reading: "text-muted-foreground",
};

// The strength order a document's participants are ranked in — implementing outranks editing
// outranks reading. The one ordering both `strongestRole` and any future role-sorted view, so the
// ranking changes in exactly one place.
export const DOCUMENT_ROLE_ORDER: DocumentRole[] = ["implementing", "editing", "reading"];

/** The strongest role among a document's participants — what the row shows at a glance. */
export function strongestRole(participants: DocumentParticipant[]): DocumentRole {
  const present = new Set(participants.map((participant) => participant.role));
  return DOCUMENT_ROLE_ORDER.find((role) => present.has(role)) ?? "reading";
}

/** Who is on this document and how, one line per participant — the row's tooltip. */
export function participantsLabel(participants: DocumentParticipant[]): string {
  return participants
    .map((participant) => `${participant.label} — ${DOCUMENT_ROLE[participant.role]}`)
    .join(", ");
}
