import { UserRound } from "lucide-react";
import { MarkdownView } from "@/components/editor/MarkdownView";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { commentAuthorLabel } from "@/lib/todo";
import { monogram } from "@/store/projects";
import type { Comment, CommentAuthor } from "@/domain";

// A comment thread's entries, each named by its author. The author is whatever the core stamped — a
// bound process's label, an external caller's label, or "unattributed" — never invented here.
// Entries are divided by hairlines rather than set on tinted plates: a fill step between two
// near-black surfaces survives one theme and disappears in the next, while a stroke reads in all of
// them. The thread's frame is the caller's, so this stays the list and nothing else.
export function CommentList({ comments }: { comments: Comment[] }) {
  if (comments.length === 0) {
    return (
      <p className="type-body px-3 py-4 text-muted-foreground">
        No comments yet. Notes left here are visible to every agent on this todo.
      </p>
    );
  }

  return (
    <ul className="flex flex-col divide-y divide-border">
      {comments.map((comment) => (
        <li key={comment.id} className="flex gap-2.5 px-3 py-2.5">
          <CommentAvatar author={comment.author} />
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="type-label font-[550] text-muted-foreground">
              {commentAuthorLabel(comment.author)}
            </span>
            <CommentBody comment={comment} />
          </div>
        </li>
      ))}
    </ul>
  );
}

// A comment is Markdown, and it goes through the same renderer the document bodies use rather than
// being printed as source. The read-only surface already carries the 13px thread size and no frame,
// so the only thing this constrains is bleed: the wrapper scrolls a wide table or code block inside
// the entry instead of widening the well, and drops the renderer's trailing margin so entries stay
// the density the divided list wants.
function CommentBody({ comment }: { comment: Comment }) {
  return (
    <div className="min-w-0 overflow-x-auto break-words [&_.tiptap-body>*:last-child]:mb-0">
      {/*
       * The renderer reads its Markdown once. A comment carries no revision to key on — only the
       * todo's document has one — so the body itself is the version, and an edited comment remounts
       * with its new text instead of showing the text it was first rendered with.
       */}
      <MarkdownView
        key={`${comment.id}:${comment.body}`}
        markdown={comment.body}
        ariaLabel={`Comment from ${commentAuthorLabel(comment.author)}`}
      />
    </div>
  );
}

// An unattributed comment gets a glyph rather than a monogram: the core stamped no author, and a
// letter cut from the word "unattributed" would read as one. Decorative either way — the author's
// name is already beside it in text.
function CommentAvatar({ author }: { author: CommentAuthor | null }) {
  return (
    <Avatar aria-hidden className="mt-px size-6 rounded-full">
      <AvatarFallback>
        {author == null ? <UserRound className="size-3" /> : monogram(author.label)}
      </AvatarFallback>
    </Avatar>
  );
}
