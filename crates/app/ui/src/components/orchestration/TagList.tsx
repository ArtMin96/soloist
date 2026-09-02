import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface TagListProps {
  tags: string[];
  className?: string;
}

// The tags an item carries, read-only, in the one muted chip style every roster row and todo row
// shares. Spans rather than a list element, because a row's header is a button and a `<ul>` is not
// phrasing content. Renders nothing for an untagged item so a row does not pay for an empty gap.
export function TagList({ tags, className }: TagListProps) {
  if (tags.length === 0) return null;
  return (
    <span data-tags className={cn("flex min-w-0 shrink items-center gap-1", className)}>
      {tags.map((tag) => (
        <Badge key={tag} data-tag={tag} variant="muted" className="min-w-0 shrink truncate">
          {tag}
        </Badge>
      ))}
    </span>
  );
}
