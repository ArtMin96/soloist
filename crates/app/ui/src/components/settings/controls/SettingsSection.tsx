import type { ReactNode } from "react";

// A titled group of setting rows: a quiet sentence-case label header over hairline-divided
// rows. Structure is drawn with the divider, not a card (DESIGN.md flat-by-default).
export function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-7">
      <h3 className="mb-1 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {title}
      </h3>
      <div className="divide-y divide-border">{children}</div>
    </section>
  );
}
