import type { ReactNode } from "react";

// A titled group of settings, in the macOS System-Settings idiom: a quiet sentence-case label
// header (with an optional explanatory line) above an inset, rounded card whose rows are split
// by inset hairline dividers. The card frame and row inset come from this one place; rows just
// supply their vertical padding, so every panel's grouping stays identical.
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
    <section className="mb-6">
      <h3 className="px-1 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {title}
      </h3>
      {description && (
        <p className="mt-0.5 mb-1 px-1 max-w-[52ch] text-xs text-muted-foreground">{description}</p>
      )}
      <div className="mt-1.5 overflow-hidden rounded-lg border border-border bg-card px-3 divide-y divide-border">
        {children}
      </div>
    </section>
  );
}
