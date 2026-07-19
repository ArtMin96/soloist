import { commentAuthorLabel } from "@/lib/todo";
import type { Comment } from "@/domain";

// A todo's comments, each named by its author. The author is whatever the core stamped — a
// bound process's label, an external caller's label, or "unattributed" — never invented here.
export function CommentList({ comments }: { comments: Comment[] }) {
  if (comments.length === 0) return null;

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-[0.6875rem] leading-[0.875rem] font-[550] tabular-nums text-muted-foreground">
        Comments ({comments.length})
      </span>
      <ul className="flex flex-col gap-1.5">
        {comments.map((comment) => (
          <li
            key={comment.id}
            className="flex flex-col gap-0.5 rounded-md bg-sidebar-accent px-2 py-1.5"
          >
            <span className="text-[0.6875rem] text-muted-foreground">
              {commentAuthorLabel(comment.author)}
            </span>
            <span className="text-[0.8125rem] whitespace-pre-wrap text-foreground">
              {comment.body}
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
