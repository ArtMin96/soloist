import { memo } from "react";
import { Copy, Download, Ellipsis, Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ThemeAppearance, ThemeDefinition, ThemeSelection } from "@/domain";
import { cn } from "@/lib/utils";
import { themeColorsForAppearance } from "@/theme/derive";
import { ThemeCardPreview } from "./ThemePreview";

function themePalettes(theme: ThemeDefinition) {
  const palettes: Array<{ appearance: ThemeAppearance; colors: ThemeDefinition["colors"] }> = [
    { appearance: theme.appearance, colors: theme.colors },
  ];
  const alternate: ThemeAppearance = theme.appearance === "light" ? "dark" : "light";
  const colors = themeColorsForAppearance(theme, alternate);
  if (colors) palettes.push({ appearance: alternate, colors });
  return palettes;
}

// A card only restyles when its own theme or the selection changes, so it stays out of the renders
// the panel does while a live draft is being edited.
export const ThemeCard = memo(function ThemeCard({
  theme,
  selected,
  onSelect,
  onDuplicate,
  onEdit,
  onCopy,
  onExport,
  onRemove,
}: {
  theme: ThemeDefinition;
  selected: ThemeSelection;
  onSelect: (themeId: string, appearance: ThemeAppearance) => void;
  onDuplicate: (themeId: string) => void;
  onEdit: (theme: ThemeDefinition) => void;
  onCopy: (themeId: string) => void;
  onExport: (themeId: string) => void;
  onRemove: (theme: ThemeDefinition) => void;
}) {
  const custom = theme.source === "custom";
  const palettes = themePalettes(theme);

  return (
    <article className="flex min-h-28 flex-col gap-3 rounded-lg border border-border bg-card p-3">
      <div className={cn("grid gap-2", palettes.length === 1 ? "grid-cols-1" : "grid-cols-2")}>
        {palettes.map(({ appearance, colors }) => {
          const isSelected = selected[appearance] === theme.id;
          return (
            <button
              key={appearance}
              type="button"
              className="min-w-0 rounded-lg outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card"
              aria-label={`Use ${theme.name} for ${appearance}`}
              aria-pressed={isSelected}
              onClick={() => onSelect(theme.id, appearance)}
            >
              <ThemeCardPreview palette={colors} appearance={appearance} selected={isSelected} />
            </button>
          );
        })}
      </div>
      <div className="mt-auto flex min-w-0 items-center gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-medium text-card-foreground">{theme.name}</div>
          {theme.author && (
            <div className="truncate text-[0.6875rem] text-muted-foreground">by {theme.author}</div>
          )}
        </div>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon-sm" aria-label={`Actions for ${theme.name}`}>
              <Ellipsis />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-40">
            {custom && (
              <DropdownMenuItem onSelect={() => onEdit(theme)}>
                <Pencil /> Edit
              </DropdownMenuItem>
            )}
            <DropdownMenuItem onSelect={() => onDuplicate(theme.id)}>
              <Copy /> Duplicate
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onCopy(theme.id)}>
              <Copy /> Copy JSON
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => onExport(theme.id)}>
              <Download /> Export
            </DropdownMenuItem>
            {custom && (
              <DropdownMenuItem variant="destructive" onSelect={() => onRemove(theme)}>
                <Trash2 /> Remove
              </DropdownMenuItem>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </article>
  );
});
