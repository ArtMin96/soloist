// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render as renderRaw, screen } from "@testing-library/react";
import { MessagesPanel } from "@/components/orchestration/MessagesPanel";
import { TooltipProvider } from "@/components/ui/tooltip";
import { MAX_RETAINED_MESSAGES, VISIBLE_MESSAGE_WINDOW } from "@/store/messagePanel";
import type { AgentMessageKind, AgentMessageRecord, AgentNode } from "@/domain";

afterEach(cleanup);

// The app mounts one TooltipProvider at its root; a panel rendered in isolation needs its own.
function render(ui: React.ReactNode) {
  return renderRaw(<TooltipProvider>{ui}</TooltipProvider>);
}

function record(
  id: number,
  overrides: Partial<{ kind: AgentMessageKind; body: string; truncated: boolean }> = {},
): AgentMessageRecord {
  return {
    delivery: {
      message: {
        id,
        project: 1,
        sender: 1,
        recipient: 2,
        kind: overrides.kind ?? "direct",
        body: overrides.body ?? `body-${id}`,
        todo_id: null,
      },
      outcome: "queued",
    },
    at_unix_millis: 1_700_000_000_000 + id,
    truncated: overrides.truncated ?? false,
  };
}

function records(count: number): AgentMessageRecord[] {
  return Array.from({ length: count }, (_, index) => record(index));
}

const AGENTS: AgentNode[] = [
  { id: 1, parent: null, label: "lead", kind: "Agent", status: "Running", activity: null },
  { id: 2, parent: 1, label: "worker 2", kind: "Agent", status: "Running", activity: null },
];

describe("MessagesPanel", () => {
  it("names the tools that produce traffic when there is none yet", () => {
    render(<MessagesPanel messages={[]} agents={AGENTS} />);
    expect(screen.getByText(/No messages between agents yet/)).toBeTruthy();
    expect(screen.getByText("agent_message_send")).toBeTruthy();
  });

  it("renders sender and recipient labels rather than raw process ids", () => {
    render(<MessagesPanel messages={[record(10)]} agents={AGENTS} />);
    expect(screen.getByText("lead")).toBeTruthy();
    expect(screen.getByText("worker 2")).toBeTruthy();
    expect(screen.queryByText("#1")).toBeNull();
    expect(screen.queryByText("#2")).toBeNull();
  });

  it("shows the body, why the message exists, and how far it travelled", () => {
    render(
      <MessagesPanel
        messages={[record(10, { kind: "completion", body: "parser landed" })]}
        agents={AGENTS}
      />,
    );
    expect(screen.getByText("parser landed")).toBeTruthy();
    expect(screen.getByText("Completion")).toBeTruthy();
    expect(screen.getByText("Queued")).toBeTruthy();
  });

  it("marks a body that was shortened for display", () => {
    render(<MessagesPanel messages={[record(10, { truncated: true })]} agents={AGENTS} />);
    expect(screen.getByText(/truncated/)).toBeTruthy();
  });

  it("leaves a whole body unmarked", () => {
    render(<MessagesPanel messages={[record(10)]} agents={AGENTS} />);
    expect(screen.queryByText(/truncated/)).toBeNull();
  });

  it("says so when the conversation begins partway through", () => {
    render(<MessagesPanel messages={records(MAX_RETAINED_MESSAGES)} agents={AGENTS} />);
    expect(screen.getByText(/earlier ones have been dropped/)).toBeTruthy();
  });

  it("does not claim dropped history while the whole conversation is retained", () => {
    render(<MessagesPanel messages={records(3)} agents={AGENTS} />);
    expect(screen.queryByText(/earlier ones have been dropped/)).toBeNull();
  });

  it("holds back all but the most recent exchanges, offering the rest", () => {
    render(<MessagesPanel messages={records(VISIBLE_MESSAGE_WINDOW + 5)} agents={AGENTS} />);
    expect(screen.getByRole("button", { name: /Show 5 earlier messages/ })).toBeTruthy();
    expect(screen.queryByText("body-0")).toBeNull();
    expect(screen.getByText(`body-${VISIBLE_MESSAGE_WINDOW + 4}`)).toBeTruthy();
  });
});
