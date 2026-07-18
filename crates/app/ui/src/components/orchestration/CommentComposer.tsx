import { useState, type KeyboardEvent } from "react";
import { SendHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

interface CommentComposerProps {
  /** Posts the comment body; resolves on success (the draft clears), rejects to keep the draft. */
  onSubmit: (body: string) => Promise<void>;
}

// The todo's comment composer: a plain-text field that submits on Enter and inserts a newline on
// Shift+Enter (Solo parity). Deliberately a minimal plain-text composer, not the rich body editor —
// comments are short plain notes, and Enter-to-send is incompatible with a rich editor's Enter — so
// it reaches for the shared editor module by *not* forking it. It owns only its draft and in-flight
// state; the post routes through the board's store, which surfaces any failure and keeps the draft.
export function CommentComposer({ onSubmit }: CommentComposerProps) {
  const [value, setValue] = useState("");
  const [posting, setPosting] = useState(false);

  const canSend = value.trim() !== "" && !posting;

  const submit = () => {
    if (!canSend) return;
    setPosting(true);
    onSubmit(value)
      .then(() => setValue(""))
      .catch(() => {
        // The board surfaces the reason in the todo's error slot; keep the draft so it is not lost.
      })
      .finally(() => setPosting(false));
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  return (
    <div className="flex items-end gap-2">
      <Textarea
        value={value}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder="Add a comment…"
        aria-label="Add a comment"
        rows={1}
        className="min-h-8 flex-1 text-[0.8125rem]"
      />
      <Button
        size="icon-sm"
        onClick={submit}
        disabled={!canSend}
        aria-label="Post comment"
        title="Post comment — Enter to send, Shift+Enter for a new line"
      >
        <SendHorizontal aria-hidden />
      </Button>
    </div>
  );
}
