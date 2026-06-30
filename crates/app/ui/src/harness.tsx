/* eslint-disable react-refresh/only-export-components -- dev-only entry, not a HMR boundary */
// Dev-only visual harness. Renders the presentational primitives with mock props so their
// static states (layout, spacing, contrast, hover/selected/disabled, theme) can be inspected
// and headless-screenshotted without the Tauri IPC runtime the real app needs to populate.
// Not part of the production bundle: Vite serves /harness.html in dev, but the Tauri build's
// rollup input is index.html alone, so nothing here ships.
//
// Usage: /harness.html (gallery) · ?dark · ?view=dialog|menus|settings|audit (each combinable with &dark)
import { useState } from "react";
import ReactDOM from "react-dom/client";
import { Check, Play, RotateCw, Settings as SettingsIcon, Square, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SegmentedControl } from "@/components/SegmentedControl";
import { ProcessIndicator } from "@/components/ProcessIndicator";
import { ProcessRow } from "@/components/sidebar/ProcessRow";
import { CommandList as ProjectCommandList } from "@/components/project-settings/CommandList";
import { ScratchpadList } from "@/components/orchestration/ScratchpadList";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { SettingsTabRail } from "@/components/settings/SettingsTabRail";
import { applyDarkClass } from "@/lib/appearance";
import type {
  AgentActivity,
  ProcessView,
  ProcStatus,
  ProjectCommandView,
  ScratchpadSummary,
} from "@/domain";
import "./index.css";

const params = new URLSearchParams(window.location.search);
applyDarkClass(params.has("dark"));

const STATUSES: ProcStatus[] = [
  "Running",
  "Starting",
  "Stopped",
  "Crashed",
  "Restarting",
  "Stopping",
  "RestartExhausted",
];
const ACTIVITIES: AgentActivity[] = ["Idle", "Permission", "Thinking", "Working", "Error"];

// A labeled block in the gallery, so every group reads with the same rhythm.
function Row({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-2.5">
      <h2 className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {title}
      </h2>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </section>
  );
}

// A mock process row, enough of ProcessView for the source list to render with no store.
function proc(
  id: number,
  label: string,
  status: ProcStatus,
  extra: Partial<ProcessView> = {},
): ProcessView {
  return {
    id,
    project: 1,
    kind: "Agent",
    label,
    status,
    exit_code: null,
    requires_trust: false,
    resumable: false,
    ports: [],
    ready: "Ungated",
    ...extra,
  };
}

// Mock commands for the project-settings grouped well.
function command(over: Partial<ProjectCommandView> & { name: string }): ProjectCommandView {
  return {
    command: "npm run dev",
    working_dir: null,
    auto_start: false,
    auto_restart: false,
    restart_when_changed: [],
    visibility: "shared",
    terminal_alerts: true,
    status: null,
    ...over,
  };
}

// A no-op command-ops bag so the project command list renders without a real backend.
const NOOP_OPS = {
  edit: () => {},
  rename: () => {},
  setTerminalAlerts: () => {},
  toggleStorage: () => {},
  remove: () => {},
  add: () => Promise.resolve(),
};

function Gallery() {
  const [segment, setSegment] = useState("agents");
  const [on, setOn] = useState(true);
  const [selectedRow, setSelectedRow] = useState(1);

  const rows = [
    proc(1, "web", "Running", { ports: [5173] }),
    proc(2, "api", "Crashed"),
    proc(3, "worker", "Stopped"),
    proc(4, "build", "Stopped", { requires_trust: true }),
  ];

  return (
    <div className="min-h-screen bg-background p-8 text-foreground">
      <div className="mx-auto flex max-w-3xl flex-col gap-8">
        <h1 className="text-lg font-semibold tracking-[-0.01em]">Soloist component harness</h1>

        <Row title="Buttons">
          <Button>Primary</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="destructive">Destructive</Button>
          <Button disabled>Disabled</Button>
        </Row>

        <Row title="Button sizes & icons">
          <Button size="sm">Small</Button>
          <Button size="sm" variant="ghost" aria-label="Start">
            <Play />
          </Button>
          <Button size="icon-sm" variant="ghost" aria-label="Restart">
            <RotateCw />
          </Button>
          <Button size="icon-xs" variant="ghost" aria-label="Stop">
            <Square />
          </Button>
        </Row>

        <Row title="Segmented control">
          <SegmentedControl
            ariaLabel="View"
            value={segment}
            onChange={setSegment}
            options={[
              { value: "agents", label: "Agents" },
              { value: "todos", label: "To-dos" },
              { value: "scratchpads", label: "Scratchpads" },
              { value: "timers", label: "Timers" },
            ]}
            counts={{ timers: 2 }}
          />
        </Row>

        <Row title="Badges & pills">
          <Badge>Default</Badge>
          <Badge variant="secondary">Secondary</Badge>
          <Badge variant="muted">AUTO</Badge>
          <Badge variant="outline">solo.yml</Badge>
          <Badge variant="destructive">Invalid</Badge>
          <Badge variant="outline" className="gap-1 border-status-running/40 text-status-running">
            <Check />
            Valid
          </Badge>
          <span className="rounded-full bg-foreground/10 px-1.5 text-[0.6875rem] tabular-nums text-muted-foreground">
            12
          </span>
        </Row>

        <Row title="Switch">
          <Switch checked={on} onCheckedChange={setOn} aria-label="Toggle" />
          <Switch checked={false} onCheckedChange={() => {}} aria-label="Off" />
          <Switch checked disabled aria-label="Disabled on" />
          <Switch checked={false} disabled aria-label="Disabled off" />
        </Row>

        <Row title="Input & select">
          <Input placeholder="Command…" className="max-w-56" />
          <Select defaultValue="medium">
            <SelectTrigger className="w-40">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="small">Small</SelectItem>
              <SelectItem value="medium">Medium</SelectItem>
              <SelectItem value="large">Large</SelectItem>
            </SelectContent>
          </Select>
          <Input disabled placeholder="Disabled" className="max-w-40" />
        </Row>

        <Row title="Status indicators">
          {STATUSES.map((status) => (
            <ProcessIndicator key={status} status={status} />
          ))}
        </Row>

        <Row title="Agent activity">
          {ACTIVITIES.map((activity) => (
            <ProcessIndicator key={activity} status="Running" activity={activity} />
          ))}
        </Row>

        <Row title="Key hints">
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Kbd>↵</Kbd> launch
          </span>
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Kbd>⌥↵</Kbd> with flags
          </span>
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Kbd>esc</Kbd> close
          </span>
        </Row>

        <div>
          <h2 className="mb-2 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
            Source list (sidebar)
          </h2>
          <div className="flex w-64 flex-col gap-px rounded-lg border border-sidebar-border bg-sidebar p-2">
            {rows.map((p) => (
              <ProcessRow
                key={p.id}
                process={p}
                selected={p.id === selectedRow}
                onSelect={() => setSelectedRow(p.id)}
                onStart={() => {}}
                onStop={() => {}}
                onRestart={() => {}}
                onResume={() => {}}
                onTrust={() => {}}
              />
            ))}
          </div>
        </div>

        <div>
          <h2 className="mb-2 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
            Command palette (launch agent)
          </h2>
          <div className="w-[28rem] overflow-hidden rounded-lg border border-border bg-popover shadow-overlay">
            <Command>
              <CommandInput placeholder="Launch agent…" />
              <CommandList>
                <CommandEmpty>No agent tools configured.</CommandEmpty>
                <CommandGroup>
                  {[
                    { name: "claude", cmd: "claude", on: true },
                    { name: "codex", cmd: "codex", on: true },
                    { name: "aider", cmd: "aider --no-auto-commits", on: false },
                  ].map((t) => (
                    <CommandItem key={t.name} value={t.name}>
                      <span className="flex size-5 shrink-0 items-center justify-center rounded bg-muted text-[0.6875rem] font-medium text-muted-foreground">
                        {t.name.charAt(0)}
                      </span>
                      <span className="font-medium">{t.name}</span>
                      <code className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                        {t.cmd}
                      </code>
                      <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
                        <span
                          className={
                            t.on
                              ? "size-1.5 rounded-full bg-muted-foreground"
                              : "size-1.5 rounded-full border border-muted-foreground/50"
                          }
                        />
                        {t.on ? "installed" : "not found"}
                      </span>
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
              <div className="flex items-center gap-3 border-t px-3 py-2 text-xs text-muted-foreground">
                <span className="flex items-center gap-1.5">
                  <Kbd>↵</Kbd> launch
                </span>
                <span className="flex items-center gap-1.5">
                  <Kbd>⌥↵</Kbd> with flags
                </span>
                <span className="ml-auto min-w-0 truncate">▸ storefront</span>
              </div>
            </Command>
          </div>
        </div>

        <div className="max-w-2xl">
          <h2 className="mb-2 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
            Project commands (grouped well)
          </h2>
          <ProjectCommandList
            commands={[
              command({ name: "Web", command: "npm run dev", auto_start: true, status: "Running" }),
              command({ name: "API", command: "cargo run", status: "Crashed" }),
              command({ name: "Worker", command: "node worker.js", visibility: "local" }),
            ]}
            ops={NOOP_OPS}
          />
        </div>

        <div className="max-w-md">
          <SettingsSection title="Appearance" description="A grouped, inset settings card.">
            <div className="flex items-center justify-between py-2.5">
              <span className="text-[0.8125rem]">Follow system theme</span>
              <Switch checked={on} onCheckedChange={setOn} aria-label="Follow system" />
            </div>
            <div className="flex items-center justify-between py-2.5">
              <span className="text-[0.8125rem]">Reduce motion</span>
              <Switch checked={false} onCheckedChange={() => {}} aria-label="Reduce motion" />
            </div>
          </SettingsSection>
        </div>
      </div>
    </div>
  );
}

// The settings surface: the left rail next to a representative panel, matching the real overlay.
function SettingsView() {
  const [tab, setTab] = useState<"appearance" | "agents" | "hotkeys" | "tools">("appearance");
  const [on, setOn] = useState(true);
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex h-11 shrink-0 items-center gap-2 border-b px-4">
        <SettingsIcon className="size-4 text-muted-foreground" />
        <span className="text-[0.9375rem] font-medium tracking-[-0.005em]">Settings</span>
      </header>
      <div className="flex min-h-0 flex-1">
        <SettingsTabRail active={tab} onSelect={(id) => setTab(id as typeof tab)} />
        <div className="min-w-0 flex-1 overflow-y-auto bg-sidebar">
          <div className="mx-auto max-w-2xl px-6 py-6">
            <SettingsSection title="Theme" description="How the interface follows the OS.">
              <div className="flex items-center justify-between py-2.5">
                <span className="text-[0.8125rem]">Follow system theme</span>
                <Switch checked={on} onCheckedChange={setOn} aria-label="Follow system" />
              </div>
              <div className="flex items-center justify-between py-2.5">
                <span className="text-[0.8125rem]">Reduce motion</span>
                <Switch checked={false} onCheckedChange={() => {}} aria-label="Reduce motion" />
              </div>
            </SettingsSection>
          </div>
        </div>
      </div>
    </div>
  );
}

// Selection audit: the three source-list surfaces side by side, each with one item selected, so the
// macOS azure-tinted selection (`--sidebar-sel-fill`) can be compared pixel-to-pixel across them.
const AUDIT_PADS: ScratchpadSummary[] = [
  {
    id: 1,
    name: "research",
    revision: 4,
    objective: "Survey the auth options",
    tags: [],
    archived: false,
  },
  {
    id: 2,
    name: "plan",
    revision: 12,
    objective: "Migration steps and order",
    tags: [],
    archived: false,
  },
  { id: 3, name: "risks", revision: 1, objective: "", tags: [], archived: false },
];

function AuditView() {
  const sidebarRows = [
    proc(1, "web", "Running", { ports: [5173] }),
    proc(2, "api", "Crashed"),
    proc(3, "worker", "Stopped"),
  ];
  return (
    <div className="min-h-screen bg-background p-8 text-foreground">
      <div className="flex flex-wrap items-start gap-10">
        <AuditColumn label="Sidebar row">
          <div className="flex w-56 flex-col gap-px rounded-lg border border-sidebar-border bg-sidebar p-2">
            {sidebarRows.map((p) => (
              <ProcessRow
                key={p.id}
                process={p}
                selected={p.id === 1}
                onSelect={() => {}}
                onStart={() => {}}
                onStop={() => {}}
                onRestart={() => {}}
                onResume={() => {}}
                onTrust={() => {}}
              />
            ))}
          </div>
        </AuditColumn>
        <AuditColumn label="Settings rail">
          <div className="rounded-lg border border-sidebar-border bg-sidebar">
            <SettingsTabRail active="appearance" onSelect={() => {}} />
          </div>
        </AuditColumn>
        <AuditColumn label="Scratchpad list">
          <div className="w-56 rounded-lg border border-sidebar-border bg-sidebar">
            <ScratchpadList scratchpads={AUDIT_PADS} selected="research" onSelect={() => {}} />
          </div>
        </AuditColumn>
      </div>
    </div>
  );
}

function AuditColumn({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-2.5">
      <h2 className="text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
        {label}
      </h2>
      {children}
    </div>
  );
}

// Open menu surfaces, so the floating dropdown/select share one overlay shadow + item language.
function MenusView() {
  return (
    <div className="min-h-screen bg-background p-8 text-foreground">
      <div className="flex items-start gap-12">
        <DropdownMenu open>
          <DropdownMenuTrigger asChild>
            <Button variant="outline">Project actions</Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-56">
            <DropdownMenuLabel>Storefront</DropdownMenuLabel>
            <DropdownMenuItem>
              <Play />
              Start all
            </DropdownMenuItem>
            <DropdownMenuItem>
              <RotateCw />
              Restart running
            </DropdownMenuItem>
            <DropdownMenuItem>
              <Square />
              Stop all
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuCheckboxItem checked>Hide empty sections</DropdownMenuCheckboxItem>
            <DropdownMenuItem>
              <SettingsIcon />
              Project settings
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive">
              <Trash2 />
              Remove project
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <div className="pt-1">
          <Select open defaultValue="medium">
            <SelectTrigger className="w-44">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="small">Small</SelectItem>
              <SelectItem value="medium">Medium</SelectItem>
              <SelectItem value="large">Large</SelectItem>
              <SelectItem value="extra">Extra large</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>
  );
}

function DialogView() {
  return (
    <div className="min-h-screen bg-background">
      <Dialog open>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Trust this command?</DialogTitle>
            <DialogDescription>
              Soloist detected a new command in solo.yml. Review it before allowing Soloist to run
              it.
            </DialogDescription>
          </DialogHeader>
          <div className="rounded-md border border-border bg-muted/50 p-3 font-mono text-[0.8125rem]">
            npm run dev
          </div>
          <DialogFooter>
            <Button variant="ghost">Not now</Button>
            <Button>Trust command</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

const view = params.get("view");
const Root =
  view === "dialog"
    ? DialogView
    : view === "menus"
      ? MenusView
      : view === "settings"
        ? SettingsView
        : view === "audit"
          ? AuditView
          : Gallery;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <TooltipProvider delayDuration={400}>
    <Root />
  </TooltipProvider>,
);
