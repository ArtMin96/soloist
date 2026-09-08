import type { ReactNode } from "react";
import {
  DetailBackButton,
  DetailBody,
  DetailNotice,
  DetailPaneHeader,
} from "@/components/common/DetailPane";

/** The handle naming which subject is being created in the detail panel. */
export const CREATE_PANE_ATTRIBUTE = "data-create-pane";

interface CreatePaneProps {
  subject: string;
  destination: string;
  error: string | null;
  onBack: () => void;
  children: ReactNode;
}

/** The shared detail-panel frame for creating a board item. */
export function CreatePane({ subject, destination, error, onBack, children }: CreatePaneProps) {
  return (
    <article {...{ [CREATE_PANE_ATTRIBUTE]: subject }} className="flex h-full min-h-0 flex-col">
      <DetailPaneHeader
        back={<DetailBackButton destination={destination} onClick={onBack} />}
        title={`New ${subject}`}
      />
      {error && <DetailNotice message={error} />}
      <DetailBody>{children}</DetailBody>
    </article>
  );
}
