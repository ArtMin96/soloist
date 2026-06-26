import type { ReactNode } from "react";

// A titled group of setting rows: a quiet sentence-case label header (with an optional
// explanatory line) over hairline-divided rows. Structure is drawn with the divider, not a
// card (DESIGN.md flat-by-default).
export function SettingsSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="mb-7">
      <h3 className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {title}
      </h3>
      {description && (
        <p className="mt-0.5 mb-1 max-w-[52ch] text-xs text-muted-foreground">{description}</p>
      )}
      <div className="mt-1 divide-y divide-border">{children}</div>
    </section>
  );
}
