import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { NullableSelect } from "@/components/settings/controls/NullableSelect";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  assistToolOptions,
  detectionLabel,
  draftCapableTools,
  offersAssistTool,
  toolInvocation,
  UNCHECKED_HINT,
} from "@/lib/agents";
import { useAgentTools } from "@/store/useAgentTools";
import { useAssistSettings } from "@/store/useAssistSettings";

const ASSIST_SECTION_DESCRIPTION =
  "Soloist can run one of your agent CLIs to draft a commit message or a pull request description for you to edit.";
const ASSIST_LABEL = "Draft text with";
const ASSIST_DESCRIPTION =
  "Runs the tool once, without a terminal. Only installed tools Soloist can run headless are offered.";
const NO_ASSIST_AVAILABLE = "No tool is available to draft with.";

// What to install to get one, named from the registry rows above rather than from a list kept here.
function installHint(capable: string[]): string {
  return `${NO_ASSIST_AVAILABLE} Install one of ${capable.join(", ")}, then run Detect above.`;
}

// The Agents tab: the read-only registry of detectable agent CLIs Soloist can launch, and which of
// them may be run once to draft text. Pure presentation over the projected read model.
export function AgentsPanel() {
  const { tools, detect, failure } = useAgentTools();
  const assist = useAssistSettings();
  const assistOptions = assistToolOptions(tools);
  const capable = draftCapableTools(tools);
  // Nothing to pick, and something to name: say what to install, so a dropdown whose only entry
  // turns the feature off explains itself rather than reading as decoration. Said beside the picker
  // and never instead of it — this is exactly the state a selection gets stuck in, and a stored tool
  // is only legible, and only clearable, while the control holding it is on screen. While the sweep
  // is still resolving capabilities there is nothing to name, so the plain description stands.
  const assistDescription =
    !offersAssistTool(assistOptions) && capable.length > 0
      ? installHint(capable)
      : ASSIST_DESCRIPTION;

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
        {failure && (
          <div className="py-3">
            <AdvisoryNotice>{failure}</AdvisoryNotice>
          </div>
        )}
        <SettingRow label="Detect installed tools" description="Re-probe your PATH for agent CLIs.">
          <Button variant="outline" size="sm" onClick={detect}>
            Detect
          </Button>
        </SettingRow>
      </SettingsSection>

      <SettingsSection title="Assist" description={ASSIST_SECTION_DESCRIPTION}>
        <SettingRow label={ASSIST_LABEL} description={assistDescription}>
          <NullableSelect<string>
            value={assist.value.tool}
            options={assistOptions}
            onValueChange={(tool) => assist.update({ tool })}
            ariaLabel={ASSIST_LABEL}
            className="w-48"
          />
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
