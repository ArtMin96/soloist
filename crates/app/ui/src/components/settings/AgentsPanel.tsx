import { useCallback, useEffect, useState } from "react";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SUMMARIZER_OFF, toolInvocation } from "@/lib/agents";
import type { Option } from "@/lib/appearance";
import { useAgentSettings } from "@/store/useAgentSettings";
import { useAgentTools } from "@/store/useAgentTools";

// The Agents tab: the read-only registry of detectable agent CLIs (Phase-7), and the
// auto-summarization opt-in (tool + model, OFF by default — the locked decision; the core never
// hard-depends on an LLM). Pure presentation over the projected read models.
export function AgentsPanel() {
  const { tools, detect } = useAgentTools();
  const { value: settings, update } = useAgentSettings();

  // The model commits on blur / Enter, not per keystroke; local state tracks the field and is
  // re-synced whenever the stored value changes.
  const [model, setModel] = useState(settings.summarizer_model ?? "");
  useEffect(() => setModel(settings.summarizer_model ?? ""), [settings.summarizer_model]);

  const commitModel = useCallback(() => {
    const trimmed = model.trim();
    const next = trimmed === "" ? null : trimmed;
    if (next !== settings.summarizer_model) update({ ...settings, summarizer_model: next });
  }, [model, settings, update]);

  // Off plus each configured tool; a stored tool absent from the detected list is kept as an
  // option so the current selection always renders.
  const toolOptions: Option<string>[] = [
    { value: SUMMARIZER_OFF, label: "Off" },
    ...tools.map((detected) => ({ value: detected.tool.name, label: detected.tool.name })),
  ];
  if (settings.summarizer_tool && !tools.some((d) => d.tool.name === settings.summarizer_tool)) {
    toolOptions.push({ value: settings.summarizer_tool, label: settings.summarizer_tool });
  }

  const summarizerOff = settings.summarizer_tool === null;

  return (
    <div className="flex flex-col">
      <SettingsSection
        title="Agent tools"
        description="The agent CLIs Soloist can launch, detected from your PATH."
      >
        {tools.map((detected) => (
          <div
            key={detected.tool.name}
            className="flex items-center justify-between gap-6 py-3"
          >
            <div className="min-w-0">
              <div className="text-[0.8125rem] text-foreground">{detected.tool.name}</div>
              <p className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
                {toolInvocation(detected.tool)}
              </p>
            </div>
            <Badge variant={detected.installed ? "outline" : "muted"} className="shrink-0">
              {detected.installed ? "Installed" : "Not found"}
            </Badge>
          </div>
        ))}
        <SettingRow label="Detect installed tools" description="Re-probe your PATH for agent CLIs.">
          <Button variant="outline" size="sm" onClick={detect}>
            Detect
          </Button>
        </SettingRow>
      </SettingsSection>

      <SettingsSection
        title="Auto-summarization"
        description="Write a one-line summary when an agent or terminal goes idle. Off by default; choose a tool to opt in."
      >
        <SettingRow label="Summarizer tool" description="The agent CLI that writes the summary.">
          <SettingSelect
            value={settings.summarizer_tool ?? SUMMARIZER_OFF}
            options={toolOptions}
            onValueChange={(value) =>
              update({ ...settings, summarizer_tool: value === SUMMARIZER_OFF ? null : value })
            }
            ariaLabel="Summarizer tool"
            className="w-44"
          />
        </SettingRow>
        <SettingRow label="Model" description="Passed to the tool via its model flag (e.g. haiku).">
          <Input
            value={model}
            onChange={(event) => setModel(event.target.value)}
            onBlur={commitModel}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            disabled={summarizerOff}
            placeholder="default"
            aria-label="Summarizer model"
            className="w-44"
          />
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
