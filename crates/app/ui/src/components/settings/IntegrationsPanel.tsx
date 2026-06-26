import { CodeBlock } from "@/components/settings/controls/CodeBlock";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Switch } from "@/components/ui/switch";
import {
  HTTP_API_BASE_URL,
  HTTP_API_ENDPOINTS,
  MCP_CLIENT_CONFIG,
  MCP_TOOL_GROUPS,
} from "@/lib/integrations";
import { useMcpToolGroups } from "@/store/useMcpToolGroups";

// The Integrations tab: which MCP tool groups the soloist-mcp server exposes (the enforced G10
// surface), plus read-only setup for the stdio MCP transport (no network port — D4) and the
// local HTTP API (Phase 10). Pure presentation over the projected enablement read model.
export function IntegrationsPanel() {
  const { groups, setGroup } = useMcpToolGroups();

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
        description="Register soloist-mcp as a stdio server in your client (Claude Code, Codex, OpenCode, Cursor, …)."
      >
        <div className="py-3">
          <CodeBlock>{MCP_CLIENT_CONFIG}</CodeBlock>
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
