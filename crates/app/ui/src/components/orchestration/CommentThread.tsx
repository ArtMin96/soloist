import { Section } from "@/components/common/Section";
import { Well } from "@/components/common/Well";
import { CommentComposer } from "@/components/orchestration/CommentComposer";
import { CommentList } from "@/components/orchestration/CommentList";
import { Separator } from "@/components/ui/separator";
import type { Comment } from "@/domain";

interface CommentThreadProps {
  comments: Comment[];
  /** Posts a comment; resolves on success, rejects to keep the composer's draft. */
  onComment: (body: string) => Promise<void>;
}

// A subject's discussion: the thread and the composer inside one well, so the composer reads as the
// end of the conversation rather than a control that happens to sit below it. The separator between
// them is the only thing dividing a read region from a write one, so it stays a rule rather than a
// gap the well's own fill would swallow.
export function CommentThread({ comments, onComment }: CommentThreadProps) {
  return (
    <Section title="Comments" aside={comments.length}>
      <Well className="overflow-hidden">
        <CommentList comments={comments} />
        <Separator />
        <div className="p-2">
          <CommentComposer onSubmit={onComment} />
        </div>
      </Well>
    </Section>
  );
}
