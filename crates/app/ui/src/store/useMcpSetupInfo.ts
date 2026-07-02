import { useState } from "react";
import { mcpSetupInfo } from "@/api";
import { DEFAULT_MCP_SETUP_INFO } from "@/lib/integrations";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { McpSetupInfo } from "@/domain";

// The snippet facts read model: the helper command and data-directory resolution the
// Integrations panel renders client snippets from. Read-only — loads once, keeping the safe
// fallback (bare helper name, default data dir) until the app answers.
export function useMcpSetupInfo(): McpSetupInfo {
  const [info, setInfo] = useState<McpSetupInfo>(DEFAULT_MCP_SETUP_INFO);

  useLoadOnce(mcpSetupInfo, setInfo);

  return info;
}
