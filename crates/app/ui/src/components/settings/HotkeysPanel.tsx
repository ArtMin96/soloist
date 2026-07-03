import { useEffect, useState } from "react";
import { AlertTriangle, RotateCcw, Search, X } from "lucide-react";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import {
  bindingFromEvent,
  formatChord,
  HOTKEY_ACTION_LABELS,
  HOTKEY_SCOPE_DESCRIPTIONS,
  HOTKEY_SCOPE_LABELS,
  HOTKEY_SCOPE_ORDER,
} from "@/lib/hotkeys";
import { cn } from "@/lib/utils";
import { useHotkeys } from "@/store/hotkeysContext";
import type { HotkeyAction, HotkeyBindingView } from "@/domain";

// The Hotkeys tab: a searchable, scoped keymap. Click a row to capture a new chord, hover and
// press the × to disable it, reset a row or all to the code defaults; same-scope collisions are
// flagged. The keymap is the live one (via the provider), so an edit takes effect immediately.
export function HotkeysPanel() {
  const { bindings, remap, disable, reset, resetAll } = useHotkeys();
  const [query, setQuery] = useState("");
  const [capturing, setCapturing] = useState<HotkeyAction | null>(null);

  // While capturing, intercept keydowns in the capture phase so the chord is recorded rather than
  // dispatched by the global handler. Escape cancels; a modifier alone keeps waiting.
  useEffect(() => {
    if (!capturing) return;
    const action = capturing;
    function onKey(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setCapturing(null);
        return;
      }
      const binding = bindingFromEvent(event);
      if (!binding) return;
      remap(action, binding);
      setCapturing(null);
    }
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing, remap]);

  const q = query.trim().toLowerCase();
  const groups = HOTKEY_SCOPE_ORDER.flatMap((scope) => {
    const rows = bindings.filter(
      (row) =>
        row.scope === scope && (!q || HOTKEY_ACTION_LABELS[row.action].toLowerCase().includes(q)),
    );
    return rows.length > 0 ? [{ scope, rows }] : [];
  });

  return (
    <div className="flex flex-col">
      <div className="mb-3 flex items-center gap-2">
        <div className="relative flex-1">
          <Search
            aria-hidden
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search shortcuts"
            aria-label="Search shortcuts"
            className="h-8 pl-8"
          />
        </div>
        <Button variant="outline" size="sm" onClick={resetAll}>
          Reset all
        </Button>
      </div>
      <p className="mb-4 max-w-[54ch] text-xs text-muted-foreground">
        Click a shortcut to change it, or hover and press the × to disable it. The same key can be
        used in different scopes.
      </p>

      {groups.map((group) => (
        <SettingsSection
          key={group.scope}
          title={HOTKEY_SCOPE_LABELS[group.scope]}
          description={HOTKEY_SCOPE_DESCRIPTIONS[group.scope]}
        >
          {group.rows.map((row) => (
            <HotkeyRow
              key={row.action}
              row={row}
              capturing={capturing === row.action}
              onCapture={() => setCapturing(row.action)}
              onDisable={() => disable(row.action)}
              onReset={() => reset(row.action)}
            />
          ))}
        </SettingsSection>
      ))}

      {groups.length === 0 && (
        <p className="py-8 text-center text-sm text-muted-foreground">
          No shortcuts match your search.
        </p>
      )}
    </div>
  );
}

function HotkeyRow({
  row,
  capturing,
  onCapture,
  onDisable,
  onReset,
}: {
  row: HotkeyBindingView;
  capturing: boolean;
  onCapture: () => void;
  onDisable: () => void;
  onReset: () => void;
}) {
  const label = HOTKEY_ACTION_LABELS[row.action];

  return (
    <div className="group/hotkey flex items-center justify-between gap-4 py-2.5">
      <div className="flex min-w-0 items-center gap-2">
        <span className="truncate text-[0.8125rem] text-foreground">{label}</span>
        {row.conflict && (
          <Badge variant="muted" className="gap-1">
            <AlertTriangle aria-hidden />
            Conflict
          </Badge>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {!row.is_default && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Reset ${label} shortcut`}
            onClick={onReset}
            className="opacity-0 transition-opacity group-hover/hotkey:opacity-100 focus-visible:opacity-100"
          >
            <RotateCcw />
          </Button>
        )}
        {row.binding && (
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Disable ${label} shortcut`}
            onClick={onDisable}
            className="opacity-0 transition-opacity group-hover/hotkey:opacity-100 focus-visible:opacity-100"
          >
            <X />
          </Button>
        )}
        <button
          type="button"
          onClick={onCapture}
          aria-label={`Change ${label} shortcut`}
          className={cn(
            "inline-flex h-7 min-w-[5rem] items-center justify-center gap-1 rounded-md border px-2 transition-colors",
            "focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
            capturing ? "border-ring bg-muted text-foreground" : "border-border hover:bg-muted",
          )}
        >
          {capturing ? (
            <span className="text-[0.8125rem] text-muted-foreground">Press keys…</span>
          ) : row.binding ? (
            formatChord(row.binding).map((token) => <Kbd key={token}>{token}</Kbd>)
          ) : (
            <span className="text-[0.8125rem] text-muted-foreground italic">Disabled</span>
          )}
        </button>
      </div>
    </div>
  );
}
