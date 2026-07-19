import { AdvisoryNotice } from "@/components/AdvisoryNotice";
import { CodeBlock } from "@/components/settings/controls/CodeBlock";
import { SettingRow } from "@/components/settings/controls/SettingRow";
import { SettingsSection } from "@/components/settings/controls/SettingsSection";
import { Input } from "@/components/ui/input";
import {
  TEMPLATE_PREVIEW_DESCRIPTION,
  TEMPLATE_PREVIEW_NO_PLACEHOLDERS,
  unfilledNotice,
  unknownNotice,
} from "@/lib/templates";
import type { RenderedPrompt } from "@/domain";

interface TemplatePreviewProps {
  /** The placeholder names the core derived from the body, in first-appearance order. */
  placeholders: string[];
  /** The value typed per placeholder name; a name absent here has no value supplied. */
  values: Record<string, string>;
  onValueChange: (placeholder: string, value: string) => void;
  /** The latest render, or null before the first one resolves. */
  rendered: RenderedPrompt | null;
  /** A refused render, surfaced verbatim from the core, or null. */
  error: string | null;
}

// The prompt a template actually produces: one value field per declared placeholder over the rendered
// output. This is the only surface that makes a template's headline concept visible — the fill-ins
// are declared in the body and, until here, were computed and shipped but never shown.
//
// The gap is reported twice on purpose, and both reports come from the core's own render result. A
// placeholder left empty keeps its literal `{{token}}` in the output, so the hole is visible in the
// text the user is already reading, *and* is named in the notice above it, so it is findable in a
// long prompt without hunting. A value naming no placeholder is the mirror case — a typo, or a marker
// edited out of the body — and would otherwise be silently dropped.
//
// Presentational: it substitutes nothing and derives nothing. Values arrive as props and every render
// is the core's.
export function TemplatePreview({
  placeholders,
  values,
  onValueChange,
  rendered,
  error,
}: TemplatePreviewProps) {
  return (
    <SettingsSection title="Preview" description={TEMPLATE_PREVIEW_DESCRIPTION}>
      {placeholders.length === 0 ? (
        <p className="py-2.5 text-xs text-muted-foreground">{TEMPLATE_PREVIEW_NO_PLACEHOLDERS}</p>
      ) : (
        placeholders.map((placeholder) => (
          <SettingRow key={placeholder} label={placeholder}>
            <Input
              value={values[placeholder] ?? ""}
              onChange={(event) => onValueChange(placeholder, event.target.value)}
              aria-label={`Value for ${placeholder}`}
              className="h-8 w-56 text-[0.8125rem]"
            />
          </SettingRow>
        ))
      )}

      <div className="flex flex-col gap-2 py-3">
        {error != null ? (
          <p className="text-[0.8125rem] text-destructive" aria-live="polite">
            {error}
          </p>
        ) : (
          rendered != null && (
            <>
              {rendered.unfilled.length > 0 && (
                <AdvisoryNotice>{unfilledNotice(rendered.unfilled)}</AdvisoryNotice>
              )}
              {rendered.unknown.length > 0 && (
                <AdvisoryNotice>{unknownNotice(rendered.unknown)}</AdvisoryNotice>
              )}
              <CodeBlock className="whitespace-pre-wrap break-words" copy={rendered.text}>
                {rendered.text}
              </CodeBlock>
            </>
          )
        )}
      </div>
    </SettingsSection>
  );
}
