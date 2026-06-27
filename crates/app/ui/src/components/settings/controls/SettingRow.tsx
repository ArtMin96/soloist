import type { ReactNode } from "react";

// One setting: its label and optional description on the left, its control on the right. The
// control names itself (an aria-label or an associated id), so the visible label stays plain text.
export function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-3">
      <div className="min-w-0">
        <div className="text-[0.8125rem] text-foreground">{label}</div>
        {description && (
          <p className="mt-0.5 max-w-[42ch] text-xs text-muted-foreground">{description}</p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
