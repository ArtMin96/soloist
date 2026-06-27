import { agentSettings, setAgentSettings } from "@/api";
import { DEFAULT_AGENT_SETTINGS } from "@/lib/agents";
import { useSettingsResource } from "@/store/useSettingsResource";

// The Agents tab's auto-summarization read model (tool + model, OFF by default), auto-saved on
// change. The single place the Agents document is bound to its facade getter/setter and default.
export function useAgentSettings() {
  return useSettingsResource(agentSettings, setAgentSettings, DEFAULT_AGENT_SETTINGS);
}
