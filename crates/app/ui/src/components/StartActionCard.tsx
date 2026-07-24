import type { LucideIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { EmptyMedia } from "@/components/ui/empty";

export type StartAction =
  | {
      title: string;
      description: string;
      icon: LucideIcon;
      availability: "available";
      onSelect: () => void;
    }
  | {
      title: string;
      description: string;
      icon: LucideIcon;
      availability: "coming-soon";
    };

interface StartActionCardProps {
  action: StartAction;
}

// One reusable project-entry card. Availability is a discriminated union so future cards cannot
// look actionable without behavior. The shared header grid keeps every icon, title, and action on
// the same alignment as these paths become functional.
export function StartActionCard({ action }: StartActionCardProps) {
  const Icon = action.icon;
  const available = action.availability === "available";

  return (
    <Card size="sm" className="h-full rounded-lg text-left shadow-none">
      <CardHeader>
        <EmptyMedia variant="icon">
          <Icon aria-hidden />
        </EmptyMedia>
        <CardTitle>{action.title}</CardTitle>
        <CardAction>
          {available ? (
            <Button
              variant="outline"
              size="sm"
              onClick={action.onSelect}
              aria-label={`${action.title}. ${action.description}`}
            >
              Open
            </Button>
          ) : (
            <Badge variant="muted">Coming soon</Badge>
          )}
        </CardAction>
      </CardHeader>
      <CardContent className="flex-1">
        <CardDescription>{action.description}</CardDescription>
      </CardContent>
    </Card>
  );
}
