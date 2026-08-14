import { useCallback, useState } from "react";
import { ArrowRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  agentLabels,
  formatMessageTime,
  isAtRetentionCeiling,
  MAX_RETAINED_MESSAGES,
  MESSAGE_KIND,
  MESSAGE_OUTCOME,
  windowMessages,
} from "@/store/messagePanel";
import type { AgentMessageRecord, AgentNode } from "@/domain";

interface Props {
  messages: AgentMessageRecord[];
  /** The flat agent list from the orchestration snapshot — for label lookups. */
  agents: AgentNode[];
}

// The messages panel: one chronological stream of the agent-to-agent traffic in this project,
// oldest first, so a human can read what one agent told another without opening a terminal. Each
// row carries the routing (sender → recipient), why the message exists, how far it has travelled,
// and the body. Read-only — composing a message is not a surface here. The retained log is bounded
// by the core, and both bounds are stated rather than hidden: a conversation that begins partway
// through says so, and a body cut for display says so.
export function MessagesPanel({ messages, agents }: Props) {
  const [expanded, setExpanded] = useState(false);
  const labelOf = agentLabels(agents);
  const showAll = useCallback(() => setExpanded(true), []);

  if (messages.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-center">
        <p className="max-w-[32ch] text-[0.8125rem] text-muted-foreground">
          No messages between agents yet.{" "}
          <span className="font-mono text-[0.75rem]">agent_message_send</span> and{" "}
          <span className="font-mono text-[0.75rem]">agent_message_broadcast</span> route them via
          MCP.
        </p>
      </div>
    );
  }

  const { visible, hidden } = windowMessages(messages, expanded);

  return (
    <div
      role="log"
      aria-label="Agent messages"
      className="flex h-full flex-col divide-y overflow-auto"
    >
      {isAtRetentionCeiling(messages) && (
        <p className="shrink-0 px-3 py-1.5 text-[0.6875rem] text-muted-foreground">
          Retaining the most recent {MAX_RETAINED_MESSAGES} exchanges — earlier ones have been
          dropped.
        </p>
      )}
      {hidden > 0 && (
        <div className="shrink-0 px-3 py-1.5">
          <Button variant="ghost" className="h-6 px-1.5 text-[0.75rem]" onClick={showAll}>
            Show {hidden} earlier {hidden === 1 ? "message" : "messages"}
          </Button>
        </div>
      )}
      {visible.map((record) => (
        <MessageRow key={record.delivery.message.id} record={record} labelOf={labelOf} />
      ))}
    </div>
  );
}

// ── Single message row ───────────────────────────────────────────────────────────────────────────

interface RowProps {
  record: AgentMessageRecord;
  labelOf: (id: number) => string;
}

function MessageRow({ record, labelOf }: RowProps) {
  const { message, outcome } = record.delivery;

  return (
    <article className="flex min-h-7 flex-col gap-0 px-3 py-1.5">
      {/* Routing row */}
      <div className="flex items-center gap-2">
        <Badge variant="outline" className="shrink-0">
          {MESSAGE_KIND[message.kind]}
        </Badge>
        <span className="flex min-w-0 items-center gap-1 text-[0.75rem]">
          <span className="truncate">{labelOf(message.sender)}</span>
          <ArrowRight className="size-3 shrink-0 text-muted-foreground" aria-label="to" />
          <span className="truncate">{labelOf(message.recipient)}</span>
        </span>

        <div className="flex-1" />

        <span className="shrink-0 text-[0.6875rem] text-muted-foreground">
          {MESSAGE_OUTCOME[outcome]}
        </span>
        <time
          className="shrink-0 font-mono text-[0.6875rem] tabular-nums text-muted-foreground"
          dateTime={new Date(record.at_unix_millis).toISOString()}
        >
          {formatMessageTime(record.at_unix_millis)}
        </time>
      </div>

      {/* Body */}
      <p className="whitespace-pre-wrap break-words text-[0.8125rem]">
        {message.body}
        {record.truncated && <TruncationMark />}
      </p>
    </article>
  );
}

function TruncationMark() {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="ml-1 cursor-default text-[0.6875rem] text-muted-foreground">
          … truncated
        </span>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-[40ch] text-[0.75rem]">
        Only this record was shortened for display. The agent received the whole message.
      </TooltipContent>
    </Tooltip>
  );
}
