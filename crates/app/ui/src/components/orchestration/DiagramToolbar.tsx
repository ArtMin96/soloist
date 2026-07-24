import { Copy, Download, FileCode2, FileImage, Maximize2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DiagramCanvas } from "@/components/mermaid/DiagramCanvas";
import { exportDiagramFile } from "@/api";
import {
  copyDiagramSource,
  copyDiagramSvg,
  DIAGRAM_EXPORT_FILE,
  DIAGRAM_THEME_LABELS,
  DIAGRAM_THEME_VALUES,
  diagramExportBytes,
  readDiagramTheme,
  setDiagramTheme,
  type DiagramExportFormat,
  type DiagramTheme,
} from "@/lib/mermaid";
import { humanizeName } from "@/lib/humanize";

// The value the theme picker carries when there is no override — the diagram follows the app's
// light/dark palette. Not a Mermaid theme, so it maps to removing the frontmatter override.
const FOLLOW_APP = "follow";

interface DiagramToolbarProps {
  /** The diagram's name handle — the export dialog's suggested filename. */
  name: string;
  /** The current draft source — the theme picker reads its override from here and exports render it. */
  source: string;
  /** Applies a rewritten source (a changed theme override) back to the editor's draft + autosave. */
  onSourceChange: (next: string) => void;
  /** Surfaces a copy/export failure (a render error, an I/O error) on the editor's error line. */
  onError: (message: string | null) => void;
}

// The diagram editor's header actions: a per-diagram theme override, a copy/export menu, and a
// fullscreen viewer. Presentational — it renders the current draft and hands changes back up. The
// theme override is stored in the source's Mermaid frontmatter (so it travels with the document and the
// renderer honours it); "Follow app" removes it so the diagram tracks the app theme.
export function DiagramToolbar({ name, source, onSourceChange, onError }: DiagramToolbarProps) {
  const theme = readDiagramTheme(source);

  const onThemeChange = (value: string) => {
    onSourceChange(setDiagramTheme(source, value === FOLLOW_APP ? null : (value as DiagramTheme)));
  };

  const guard = async (action: () => Promise<unknown>) => {
    onError(null);
    try {
      await action();
    } catch (reason) {
      onError(String(reason));
    }
  };

  const exportAs = (format: DiagramExportFormat) =>
    guard(async () => {
      const bytes = await diagramExportBytes(source, format);
      const file = DIAGRAM_EXPORT_FILE[format];
      await exportDiagramFile(name, file.extension, file.label, bytes);
    });

  return (
    <div className="flex shrink-0 items-center gap-1.5">
      <Select value={theme ?? FOLLOW_APP} onValueChange={onThemeChange}>
        <SelectTrigger size="sm" aria-label="Diagram theme" className="w-auto gap-1">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            <SelectItem value={FOLLOW_APP}>Follow app</SelectItem>
            {DIAGRAM_THEME_VALUES.map((value) => (
              <SelectItem key={value} value={value}>
                {DIAGRAM_THEME_LABELS[value]}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="sm">
            <Download aria-hidden /> Export
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => void guard(() => copyDiagramSource(source))}>
            <Copy aria-hidden /> Copy source
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void guard(() => copyDiagramSvg(source))}>
            <FileCode2 aria-hidden /> Copy SVG
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => void exportAs("svg")}>
            <FileImage aria-hidden /> Export SVG
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void exportAs("mmd")}>
            <FileCode2 aria-hidden /> Export .mmd
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void exportAs("png")}>
            <FileImage aria-hidden /> Export PNG
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog>
        <DialogTrigger asChild>
          <Button variant="ghost" size="icon-sm" aria-label="Fullscreen">
            <Maximize2 aria-hidden />
          </Button>
        </DialogTrigger>
        <DialogContent presentation="fullscreen">
          <DialogHeader className="flex h-11 shrink-0 flex-row items-center border-b px-3">
            <DialogTitle className="type-title font-[550] tracking-[var(--tracking-title)]">
              {humanizeName(name)}
            </DialogTitle>
          </DialogHeader>
          <div className="min-h-0 flex-1 p-3">
            <DiagramCanvas source={source} />
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
