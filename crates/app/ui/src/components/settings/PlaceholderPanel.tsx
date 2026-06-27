// The content for a tab without a built panel yet: a quiet, honest message — either a section
// still to come in this build, or one whose fields were never defined in the source and await
// an explicit decision (never invented).
export function PlaceholderPanel({ title, message }: { title: string; message: string }) {
  return (
    <div className="flex min-h-[16rem] flex-col items-center justify-center gap-2 text-center">
      <h2 className="text-[0.9375rem] font-medium text-foreground">{title}</h2>
      <p className="max-w-[40ch] text-[0.8125rem] text-muted-foreground">{message}</p>
    </div>
  );
}
