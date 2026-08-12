import { ToastCard, ToastLines } from "@/components/ToastCard";
import { TOAST } from "@/lib/notifications";
import { cn } from "@/lib/utils";
import type { AttentionKind } from "@/domain";

export interface ProcessToastProps {
  kind: AttentionKind;
  /** The alert's first line, as the core wrote it. */
  title: string;
  /** The alert's second line, as the core wrote it. */
  body: string;
  /** Show the process this alert came from. */
  onOpen: () => void;
  onDismiss: () => void;
}

// One in-app alert about a process. The words arrive already written — the same sentence the desktop
// would have shown — so this renders them on the shared alert card and adds only what this kind of
// alert has of its own: the state's glyph, and a body that leads to the process it came from.
export function ProcessToast({ kind, title, body, onOpen, onDismiss }: ProcessToastProps) {
  const display = TOAST[kind];

  return (
    <ToastCard
      onDismiss={onDismiss}
      mark={
        <span
          aria-hidden
          className={cn("mt-[0.3125rem] type-label leading-none", display.toneClass)}
        >
          {display.glyph}
        </span>
      }
    >
      <button
        type="button"
        onClick={onOpen}
        className="-my-0.5 min-w-0 flex-1 cursor-default rounded-md px-1.5 py-1 text-left transition-colors duration-[var(--dur-fast)] outline-none hover:bg-message-action-hover focus-visible:ring-2 focus-visible:ring-ring/50"
      >
        <ToastLines title={title} body={body} />
      </button>
    </ToastCard>
  );
}
