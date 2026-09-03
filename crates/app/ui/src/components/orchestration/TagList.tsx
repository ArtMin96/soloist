import { TagIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface TagListProps {
  tags: string[];
  className?: string;
  /** Lets chips move onto another line instead of compressing in detail-rich surfaces. */
  wrap?: boolean;
}

// The tags an item carries, read-only, in the one muted chip style every roster row and todo row
// shares. Spans rather than a list element, because a row's header is a button and a `<ul>` is not
// phrasing content. Renders nothing for an untagged item so a row does not pay for an empty gap.
export function TagList({ tags, className, wrap = false }: TagListProps) {
  if (tags.length === 0) return null;
  return (
    <span
      data-tags
      className={cn("flex min-w-0 shrink items-center gap-1", wrap && "flex-wrap", className)}
    >
      {tags.map((tag) => (
        <Badge
          key={tag}
          data-tag={tag}
          variant="muted"
          className={cn("truncate", wrap ? "max-w-full shrink-0" : "min-w-0 shrink")}
        >
          <TagIcon aria-hidden data-icon="inline-start" />
          {tag}
        </Badge>
      ))}
    </span>
  );
}
