import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { ChevronDown, ChevronUp, MousePointer2, Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel, FieldLegend, FieldSet } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { GLASS_FLOATING_SURFACE } from "@/components/ui/glass";
import type { ThemeAppearance, ThemeColorRole, ThemeFile } from "@/domain";
import { deriveThemeColors } from "@/theme/derive";
import { themeContrastWarnings } from "@/theme/accessibility";
import { THEME_COLOR_GROUPS, THEME_COLOR_ROLE_META } from "@/theme/roles";
import { ThemeColorInput } from "./ThemeColorInput";
import { highlightThemeRoleUsage, startThemeInspector } from "./themeInspector";
import { cn } from "@/lib/utils";

export function ThemeEditor({
  initialTheme,
  editing,
  onPreview,
  onCancel,
  onSave,
}: {
  initialTheme: ThemeFile;
  editing: boolean;
  onPreview: (theme: ThemeFile) => void;
  onCancel: () => void;
  onSave: (theme: ThemeFile) => Promise<void>;
}) {
  const [theme, setTheme] = useState(initialTheme);
  const [advanced, setAdvanced] = useState(false);
  const [query, setQuery] = useState("");
  const [minimized, setMinimized] = useState(false);
  const [inspecting, setInspecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const clearRoleHighlight = useRef<() => void>(() => {});

  useEffect(() => () => clearRoleHighlight.current(), []);

  useEffect(() => {
    if (!inspecting) return;
    return startThemeInspector(theme.colors, (role) => {
      setAdvanced(true);
      setQuery(THEME_COLOR_ROLE_META[role].label);
      setInspecting(false);
    });
  }, [inspecting, theme.colors]);

  const groups = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return THEME_COLOR_GROUPS;
    return THEME_COLOR_GROUPS.map((group) => ({
      ...group,
      roles: group.roles.filter((role) =>
        `${THEME_COLOR_ROLE_META[role].label} ${role}`.toLocaleLowerCase().includes(needle),
      ),
    })).filter((group) => group.roles.length > 0);
  }, [query]);
  const contrastWarnings = useMemo(() => themeContrastWarnings(theme.colors), [theme.colors]);

  // Every edit updates the form and the live-preview draft in the same event, so the panel above
  // never has to re-render a second time to catch up.
  const change = (next: ThemeFile) => {
    setTheme(next);
    onPreview(next);
  };

  const changeAppearance = (appearance: ThemeAppearance) => {
    change({
      ...theme,
      appearance,
      colors: deriveThemeColors(appearance, theme.colors.canvas, theme.colors.accent),
    });
  };

  const changeSeed = (role: "canvas" | "accent", value: string) => {
    const background = role === "canvas" ? value : theme.colors.canvas;
    const accent = role === "accent" ? value : theme.colors.accent;
    change({ ...theme, colors: deriveThemeColors(theme.appearance, background, accent) });
  };

  const changeRole = (role: ThemeColorRole, value: string) => {
    change({ ...theme, colors: { ...theme.colors, [role]: value } });
  };

  const startDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if ((event.target as Element).closest("button, input, [role=switch]")) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY, left: offset.x, top: offset.y };
  };

  const moveDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (!drag.current) return;
    setOffset({
      x: drag.current.left + event.clientX - drag.current.x,
      y: drag.current.top + event.clientY - drag.current.y,
    });
  };

  const submit = async () => {
    if (!theme.name.trim()) {
      setError("Name your theme before saving.");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onSave({
        ...theme,
        name: theme.name.trim(),
        author: theme.author?.trim() || undefined,
      });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "The theme could not be saved.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      role="dialog"
      aria-modal="false"
      aria-labelledby="theme-editor-title"
      data-theme-editor
      className={cn(
        "fixed right-4 bottom-4 z-50 flex max-h-[calc(100vh-2rem)] min-h-14 w-[min(26rem,calc(100vw-2rem))] min-w-80 resize flex-col overflow-hidden rounded-xl border text-popover-foreground motion-reduce:transition-none",
        GLASS_FLOATING_SURFACE,
      )}
      style={{ transform: `translate(${offset.x}px, ${offset.y}px)` }}
    >
      <header
        className="flex h-12 shrink-0 cursor-move items-center gap-2 border-b border-border px-3"
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={() => (drag.current = null)}
        onPointerCancel={() => (drag.current = null)}
      >
        <h2 id="theme-editor-title" className="text-sm font-medium">
          {editing ? "Edit theme" : "Create theme"}
        </h2>
        <span className="text-xs text-muted-foreground">Live preview</span>
        <div className="ml-auto flex items-center gap-1">
          <Button
            variant={inspecting ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setInspecting((active) => !active)}
            aria-pressed={inspecting}
          >
            <MousePointer2 data-icon="inline-start" /> Inspect
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => setMinimized((value) => !value)}
            aria-label={minimized ? "Expand theme editor" : "Minimize theme editor"}
          >
            {minimized ? <ChevronUp /> : <ChevronDown />}
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onCancel}
            aria-label="Cancel theme editing"
          >
            <X />
          </Button>
        </div>
      </header>

      {!minimized && (
        <>
          <ScrollArea className="min-h-0 flex-1">
            <div className="flex flex-col gap-4 p-3">
              <Field>
                <FieldLabel htmlFor="theme-name">Theme name</FieldLabel>
                <Input
                  id="theme-name"
                  value={theme.name}
                  onChange={(event) => change({ ...theme, name: event.target.value })}
                  placeholder="e.g. Aurora"
                  autoFocus
                />
              </Field>

              <Field>
                <FieldLabel htmlFor="theme-author">Author</FieldLabel>
                <Input
                  id="theme-author"
                  value={theme.author ?? ""}
                  onChange={(event) => change({ ...theme, author: event.target.value })}
                  placeholder="Optional"
                />
              </Field>

              <FieldSet className="gap-2">
                <FieldLegend variant="label">Appearance</FieldLegend>
                <ToggleGroup
                  type="single"
                  value={theme.appearance}
                  onValueChange={(value) => value && changeAppearance(value as ThemeAppearance)}
                  variant="outline"
                  spacing={0}
                  className="grid w-full grid-cols-2"
                  aria-label="Theme appearance"
                >
                  <ToggleGroupItem value="light">Light</ToggleGroupItem>
                  <ToggleGroupItem value="dark">Dark</ToggleGroupItem>
                </ToggleGroup>
              </FieldSet>

              <div className="flex items-center gap-3">
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">Colors</div>
                  <div className="text-xs text-muted-foreground">
                    {advanced ? "Edit every interface role." : "Two colors; the rest are derived."}
                  </div>
                </div>
                {advanced && (
                  <label className="relative min-w-0 flex-1">
                    <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      value={query}
                      onChange={(event) => setQuery(event.target.value)}
                      placeholder="Filter colors"
                      aria-label="Filter theme colors"
                      className="h-8 pl-8 text-xs"
                    />
                  </label>
                )}
                <label className="flex items-center gap-2 text-xs">
                  Advanced
                  <Switch
                    checked={advanced}
                    onCheckedChange={setAdvanced}
                    aria-label="Advanced colors"
                  />
                </label>
              </div>

              {advanced ? (
                <div className="flex flex-col gap-5">
                  {groups.map((group) => (
                    <section key={group.name} className="flex flex-col gap-2">
                      <h3 className="text-xs font-medium text-foreground">{group.name} colors</h3>
                      {group.roles.map((role) => (
                        <div key={role} id={`theme-role-${role}`}>
                          <ThemeColorInput
                            label={THEME_COLOR_ROLE_META[role].label}
                            value={theme.colors[role]}
                            onChange={(value) => changeRole(role, value)}
                            onHighlight={(active) => {
                              clearRoleHighlight.current();
                              clearRoleHighlight.current = active
                                ? highlightThemeRoleUsage(theme.colors[role])
                                : () => {};
                            }}
                          />
                        </div>
                      ))}
                    </section>
                  ))}
                  {groups.length === 0 && (
                    <p className="py-8 text-center text-xs text-muted-foreground">
                      No color roles match “{query}”.
                    </p>
                  )}
                </div>
              ) : (
                <div className="flex flex-col gap-2">
                  <ThemeColorInput
                    label="Background"
                    value={theme.colors.canvas}
                    onChange={(value) => changeSeed("canvas", value)}
                  />
                  <ThemeColorInput
                    label="Accent"
                    value={theme.colors.accent}
                    onChange={(value) => changeSeed("accent", value)}
                  />
                </div>
              )}

              {contrastWarnings.length > 0 && (
                <section
                  aria-label="Accessibility warnings"
                  className="rounded-lg border border-warning bg-warning-surface p-3 text-xs text-foreground"
                >
                  <div className="font-medium">Check color contrast</div>
                  <ul className="mt-1 list-disc space-y-1 pl-4 text-muted-foreground">
                    {contrastWarnings.slice(0, 4).map((warning) => (
                      <li key={`${warning.foreground}-${warning.background}`}>
                        {warning.label} is {warning.ratio.toFixed(1)}:1; aim for at least{" "}
                        {warning.minimum}:1.
                      </li>
                    ))}
                  </ul>
                  {contrastWarnings.length > 4 && (
                    <p className="mt-1 text-muted-foreground">
                      And {contrastWarnings.length - 4} more contrast warning
                      {contrastWarnings.length - 4 === 1 ? "" : "s"}.
                    </p>
                  )}
                  <p className="mt-2">You can still save this theme.</p>
                </section>
              )}
            </div>
          </ScrollArea>

          <footer className="flex shrink-0 items-center gap-2 border-t border-border p-3">
            {error && (
              <p role="alert" className="mr-auto text-xs text-destructive">
                {error}
              </p>
            )}
            {!error && <div className="mr-auto" />}
            <Button variant="ghost" size="sm" onClick={onCancel}>
              Cancel
            </Button>
            <Button size="sm" onClick={() => void submit()} disabled={saving}>
              {saving ? "Saving…" : editing ? "Save changes" : "Create theme"}
            </Button>
          </footer>
        </>
      )}
    </section>
  );
}
