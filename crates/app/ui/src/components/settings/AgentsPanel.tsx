import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { detectionLabel, toolInvocation, UNCHECKED_HINT } from "@/lib/agents";
import { useAgentTools } from "@/store/useAgentTools";

// The Agents tab: the read-only registry of detectable agent CLIs Soloist can launch. Pure
// presentation over the projected read model.
export function AgentsPanel() {
  const { tools, detect } = useAgentTools();

  return (
    <div className="flex flex-col">
      <SettingsSection
        title="Agent tools"
        description="The agent CLIs Soloist can launch, detected from your PATH."
      >
        {tools.map((detected) => (
          <div key={detected.tool.name} className="flex items-center justify-between gap-6 py-3">
            <div className="min-w-0">
              <div className="text-[0.8125rem] text-foreground">{detected.tool.name}</div>
              <p className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
                {toolInvocation(detected.tool)}
              </p>
            </div>
            <Badge
              variant={detected.detection === "Installed" ? "outline" : "muted"}
              className="shrink-0 first-letter:uppercase"
              title={detected.detection === "Unknown" ? UNCHECKED_HINT : undefined}
            >
              {detectionLabel[detected.detection]}
            </Badge>
          </div>
        ))}
        <SettingRow label="Detect installed tools" description="Re-probe your PATH for agent CLIs.">
          <Button variant="outline" size="sm" onClick={detect}>
            Detect
          </Button>
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
