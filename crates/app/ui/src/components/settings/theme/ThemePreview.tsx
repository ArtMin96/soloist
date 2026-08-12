import { Check, Moon, Sun } from "lucide-react";
import { cn } from "@/lib/utils";

export interface ThemePreviewPalette {
  accent: string;
  border: string;
  canvas: string;
  chrome: string;
  sidebar: string;
  surface: string;
  surfaceRaised: string;
  text: string;
  textMuted: string;
}

export function MiniApplication({ palette }: { palette: ThemePreviewPalette }) {
  return (
    <div
      className="grid h-full grid-cols-[28%_1fr] overflow-hidden rounded-md border"
      style={{ background: palette.canvas, borderColor: palette.border, color: palette.text }}
      aria-hidden
    >
      <div className="flex flex-col gap-1 border-r p-2" style={{ borderColor: palette.border }}>
        <div className="h-1.5 w-3/4 rounded-full" style={{ background: palette.textMuted }} />
        <div className="h-1.5 rounded-full" style={{ background: palette.surface }} />
        <div className="h-1.5 rounded-full" style={{ background: palette.surface }} />
      </div>
      <div className="flex flex-col gap-2 p-2">
        <div className="h-1.5 w-2/3 rounded-full" style={{ background: palette.textMuted }} />
        <div
          className="mt-auto flex h-6 items-center rounded-md border px-2"
          style={{ background: palette.surfaceRaised, borderColor: palette.border }}
        >
          <div className="h-1.5 w-1/2 rounded-full" style={{ background: palette.textMuted }} />
          <div className="ml-auto size-2 rounded-full" style={{ background: palette.accent }} />
        </div>
      </div>
    </div>
  );
}

export function SchemePreview({
  light,
  dark,
  mode,
}: {
  light: ThemePreviewPalette;
  dark: ThemePreviewPalette;
  mode: "system" | "light" | "dark";
}) {
  if (mode === "light") return <MiniApplication palette={light} />;
  if (mode === "dark") return <MiniApplication palette={dark} />;

  return (
    <div className="grid h-full grid-cols-2 overflow-hidden rounded-md">
      <div className="min-w-0 overflow-hidden">
        <div className="h-full w-[200%]">
          <MiniApplication palette={light} />
        </div>
      </div>
      <div className="min-w-0 overflow-hidden">
        <div className="h-full w-[200%] -translate-x-1/2">
          <MiniApplication palette={dark} />
        </div>
      </div>
    </div>
  );
}

export function ThemeCardPreview({
  palette,
  appearance,
  selected,
}: {
  palette: ThemePreviewPalette;
  appearance: "light" | "dark";
  selected: boolean;
}) {
  const AppearanceIcon = appearance === "light" ? Sun : Moon;
  return (
    <span
      className={cn(
        "relative block h-20 overflow-hidden rounded-lg border p-1 shadow-sm",
        selected && "ring-2 ring-ring ring-offset-2 ring-offset-card",
      )}
      style={{ background: palette.chrome, borderColor: palette.border }}
    >
      <MiniApplication palette={palette} />
      <span
        className="absolute right-1.5 bottom-1.5 flex size-4 items-center justify-center rounded-full border shadow-sm"
        style={{
          background: palette.surfaceRaised,
          borderColor: palette.border,
          color: palette.text,
        }}
      >
        {selected ? <Check className="size-2.5" /> : <AppearanceIcon className="size-2.5" />}
      </span>
    </span>
  );
}
