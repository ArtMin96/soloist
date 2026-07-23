import { Bot, FolderOpen, GitBranch, Rocket, SquareTerminal } from "lucide-react";
import { StartActionCard, type StartAction } from "@/components/StartActionCard";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Separator } from "@/components/ui/separator";

interface StartSurfaceProps {
  hasProjects: boolean;
  onOpenProject: () => void;
  onLaunchAgent: () => void;
  notice?: string | null;
}

// The main pane's canonical starting point: stable whether the sidebar is empty or merely has no
// selection. Project actions stay separate from the session action so Launch never becomes an
// ambiguous fourth project card.
export function StartSurface({
  hasProjects,
  onOpenProject,
  onLaunchAgent,
  notice,
}: StartSurfaceProps) {
  const actions: readonly StartAction[] = [
    {
      title: "Open project",
      description: "Choose a folder already on this computer.",
      icon: FolderOpen,
      availability: "available",
      onSelect: onOpenProject,
    },
    {
      title: "Clone from URL",
      description: "Bring an existing Git repository into Soloist.",
      icon: GitBranch,
      availability: "coming-soon",
    },
    {
      title: "Quick start",
      description: "Start from a guided project setup.",
      icon: Rocket,
      availability: "coming-soon",
    },
  ];

  return (
    <Empty className="h-full overflow-y-auto rounded-none border-0 bg-background px-6 py-10">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <SquareTerminal aria-hidden />
        </EmptyMedia>
        <EmptyTitle>Start in Soloist</EmptyTitle>
        <EmptyDescription>Open a workspace or begin a new development session.</EmptyDescription>
      </EmptyHeader>

      <EmptyContent className="@container max-w-3xl gap-4">
        {notice && (
          <Card size="sm" role="status" className="w-full rounded-lg shadow-none">
            <CardHeader>
              <CardTitle>Project ready</CardTitle>
              <CardDescription>{notice}</CardDescription>
            </CardHeader>
          </Card>
        )}

        <div className="grid w-full gap-3 @2xl:grid-cols-3">
          {actions.map((action) => (
            <StartActionCard key={action.title} action={action} />
          ))}
        </div>

        <Separator />

        <Card size="sm" className="w-full rounded-lg text-left shadow-none">
          <CardHeader>
            <CardTitle>New session</CardTitle>
            <CardDescription>
              {hasProjects ? "Launch an agent or open a terminal." : "Open a project first."}
            </CardDescription>
            <CardAction>
              <Button variant="secondary" size="sm" disabled={!hasProjects} onClick={onLaunchAgent}>
                <Bot data-icon="inline-start" />
                Launch agent
              </Button>
            </CardAction>
          </CardHeader>
        </Card>
      </EmptyContent>
    </Empty>
  );
}
