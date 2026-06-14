import { Button } from "@/components/ui/button";
import { StatusBadge } from "@/components/StatusBadge";
import type { ProcessView } from "@/domain";

export interface ProcessListProps {
  processes: ProcessView[];
  onStop: (id: number) => void;
}

export function ProcessList({ processes, onStop }: ProcessListProps) {
  if (processes.length === 0) {
    return <p className="text-muted-foreground">no processes</p>;
  }
  return (
    <ul className="flex flex-col gap-1" data-testid="process-list">
      {processes.map((process) => (
        <ProcessRow key={process.id} process={process} onStop={onStop} />
      ))}
    </ul>
  );
}

function ProcessRow({ process, onStop }: { process: ProcessView; onStop: (id: number) => void }) {
  return (
    <li
      className="flex items-center justify-between gap-3 border-b py-1"
      data-process-id={process.id}
    >
      <span>
        #{process.id} {process.label}
      </span>
      <span className="flex items-center gap-3">
        <StatusBadge status={process.status} />
        <Button size="sm" variant="outline" onClick={() => onStop(process.id)}>
          Stop
        </Button>
      </span>
    </li>
  );
}
