import { Button } from "@/components/ui/button";

export interface ToolbarProps {
  onStart: () => void;
  onRefresh: () => void;
}

export function Toolbar({ onStart, onRefresh }: ToolbarProps) {
  return (
    <div className="flex gap-2">
      <Button size="sm" data-testid="spawn-demo" onClick={onStart}>
        Start demo process
      </Button>
      <Button size="sm" variant="outline" data-testid="refresh" onClick={onRefresh}>
        Refresh
      </Button>
    </div>
  );
}
