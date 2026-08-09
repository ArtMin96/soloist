import { CircleXIcon } from "lucide-react";
import { toast } from "sonner";
import { ToastCard, ToastLines } from "@/components/ToastCard";
import { TOAST_LIFETIME_MS } from "@/lib/notifications";

const REFUSED_TITLE = "Version control refused";

/**
 * Puts a refused exchange with the remote into the alert stack.
 *
 * Fetching, pulling and handing commits over are driven from the window chrome, which is one strip
 * tall and cannot grow a line to say why one was refused — so the refusal goes where everything else
 * the user has to read already goes, on the card every other alert uses.
 *
 * The mark is the drawn icon the repository surfaces already wear for something that failed, not the
 * status vocabulary's ✕: what failed here is a command, and ✕ is the word "Crashed" said in a glyph.
 *
 * Failures only: an exchange that worked announces itself by the repository changing.
 */
export function raiseRefusal(message: string): void {
  toast.custom(
    (id) => (
      <ToastCard
        role="alert"
        onDismiss={() => toast.dismiss(id)}
        mark={<CircleXIcon aria-hidden className="mt-0.5 size-3.5 shrink-0 text-destructive" />}
      >
        <div className="-my-0.5 min-w-0 flex-1 px-1.5 py-1">
          <ToastLines title={REFUSED_TITLE} body={message} />
        </div>
      </ToastCard>
    ),
    { duration: TOAST_LIFETIME_MS },
  );
}
