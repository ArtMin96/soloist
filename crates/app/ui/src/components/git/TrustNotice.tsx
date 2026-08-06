import { Button } from "@/components/ui/button";

const EXPLANATION =
  "Committing runs this repository's own hooks, so Soloist changes it only once you say so.";
const ACTION = "Trust this project";

/**
 * Why nothing in the rail can be changed yet, and the one action that changes it.
 *
 * It states the reason rather than the rule: the gate exists because a commit runs code the
 * project carries, which is the same thing trusting a command authorises.
 */
export function TrustNotice({ onTrust }: { onTrust: () => void }) {
  return (
    <div className="flex shrink-0 flex-col items-start gap-2 border-t border-sidebar-border p-3">
      <p className="text-[0.8125rem] text-muted-foreground">{EXPLANATION}</p>
      <Button size="sm" onClick={onTrust}>
        {ACTION}
      </Button>
    </div>
  );
}
