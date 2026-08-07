import { assistSettings, setAssistSettings } from "@/api";
import { DEFAULT_ASSIST } from "@/lib/agents";
import { useSettingsResource } from "@/store/useSettingsResource";

// The Assist read model: which agent tool may be run headless to draft text, auto-saved on change.
// The single place that document is bound to its facade getter/setter and pre-load default.
export function useAssistSettings() {
  return useSettingsResource(assistSettings, setAssistSettings, DEFAULT_ASSIST);
}
