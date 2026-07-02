import { useState } from "react";
import { CodeBlock } from "@/components/settings/controls/CodeBlock";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingSelect } from "@/components/settings/controls/SettingSelect";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import {
  HTTP_API_BASE_URL,
  HTTP_API_ENDPOINTS,
  MCP_CLIENTS,
  MCP_TOOL_GROUPS,
} from "@/lib/integrations";
import { useMcpSetupInfo } from "@/store/useMcpSetupInfo";
import { useMcpToolGroups } from "@/store/useMcpToolGroups";

// The Integrations tab: which MCP tool groups the soloist-mcp server exposes (the enforced G10
// surface), per-client MCP setup snippets generated from the app's resolved helper path and
// data directory, and the read-only local HTTP API surface. Pure presentation over the
// projected read models.
export function IntegrationsPanel() {
  const { groups, setGroup } = useMcpToolGroups();
  const setupInfo = useMcpSetupInfo();
  const [clientId, setClientId] = useState(MCP_CLIENTS[0].id);

  const client = MCP_CLIENTS.find((c) => c.id === clientId) ?? MCP_CLIENTS[0];
  const snippet = client.snippet(setupInfo);

  return (
    <div className="flex flex-col">
      <SettingsSection
        title="Model Context Protocol"
        description="Soloist exposes coordination tools to AI assistants over MCP. It speaks stdio — there is no network port. Choose which tool groups are served."
      >
        {MCP_TOOL_GROUPS.map((info) => (
          <SettingRow key={info.group} label={info.label} description={info.description}>
            <Switch
              checked={groups[info.group]}
              onCheckedChange={(enabled) => setGroup(info.group, enabled)}
              aria-label={info.label}
            />
          </SettingRow>
        ))}
      </SettingsSection>

      <SettingsSection
        title="MCP client setup"
        description="Pick your client and paste the generated snippet to register soloist-mcp as a stdio server."
      >
        <div className="flex flex-col gap-2 py-3">
          <div className="flex items-center justify-between gap-3">
            <p className="min-w-0 truncate font-mono text-[0.6875rem] text-muted-foreground">
              {client.configPath}
            </p>
            <SettingSelect
              value={client.id}
              options={MCP_CLIENTS.map(({ id, label }) => ({ value: id, label }))}
              onValueChange={(id) => setClientId(id as (typeof MCP_CLIENTS)[number]["id"])}
              ariaLabel="MCP client"
              className="w-40 shrink-0"
            />
          </div>
          <CodeBlock copy={snippet}>{snippet}</CodeBlock>
        </div>
      </SettingsSection>

      <SettingsSection
        title="HTTP API"
        description="A local REST API on loopback for scripts and tools. Mutations require the X-Soloist-Local-Auth header."
      >
        <div className="flex flex-col gap-3 py-3">
          <CodeBlock>{HTTP_API_BASE_URL}</CodeBlock>
          <div>
            <p className="mb-1.5 text-[0.6875rem] font-medium tracking-[0.01em] text-muted-foreground">
              {HTTP_API_ENDPOINTS.length} endpoints
            </p>
            <CodeBlock>{HTTP_API_ENDPOINTS.join("\n")}</CodeBlock>
          </div>
        </div>
      </SettingsSection>
    </div>
  );
}
