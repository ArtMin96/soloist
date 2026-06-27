import { toolDefaults, setToolDefaults } from "@/api";
import { DEFAULT_TOOL_DEFAULTS } from "@/lib/tools";
import { useSettingsResource } from "@/store/useSettingsResource";

// The Tools tab's read model: the default editor + terminal, auto-saved on change. The single
// place the Tools document is bound to its facade getter/setter and pre-load default.
export function useToolSettings() {
  return useSettingsResource(toolDefaults, setToolDefaults, DEFAULT_TOOL_DEFAULTS);
}
