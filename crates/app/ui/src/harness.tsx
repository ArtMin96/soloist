/* eslint-disable react-refresh/only-export-components -- dev-only entry, not a HMR boundary */
// Dev-only visual harness. Renders the presentational primitives with mock props so their
// static states (layout, spacing, contrast, hover/selected/disabled, theme) can be inspected
// and headless-screenshotted without the Tauri IPC runtime the real app needs to populate.
// Not part of the production bundle: Vite serves /harness.html in dev, but the Tauri build's
// rollup input is index.html alone, so nothing here ships.
//
// Usage: /harness.html (light) · /harness.html?dark · /harness.html?view=dialog[&dark]
import { useState } from "react";
import ReactDOM from "react-dom/client";
import { Play, RotateCw, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { applyDarkClass } from "@/lib/appearance";
import type { AgentActivity, ProcessView, ProcStatus } from "@/domain";
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

        <Row title="Switch">
          <Switch checked={on} onCheckedChange={setOn} aria-label="Toggle" />
          <Switch checked={false} onCheckedChange={() => {}} aria-label="Off" />
          <Switch checked disabled aria-label="Disabled on" />
          <Switch checked={false} disabled aria-label="Disabled off" />
        </Row>

        <Row title="Input">
          <Input placeholder="Command…" className="max-w-56" />
          <Input defaultValue="npm run dev" className="max-w-56" />
          <Input disabled placeholder="Disabled" className="max-w-56" />
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

        <div>
          <h2 className="mb-2 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
            Source list
          </h2>
          <div className="flex w-60 flex-col gap-px rounded-lg border border-sidebar-border bg-sidebar p-2">
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

ReactDOM.createRoot(document.getElementById("root")!).render(
  <TooltipProvider delayDuration={400}>
    {params.get("view") === "dialog" ? <DialogView /> : <Gallery />}
  </TooltipProvider>,
);
