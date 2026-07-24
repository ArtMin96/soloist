// Turning a diagram into the artifacts the toolbox copies and exports: its raw Mermaid source, its
// rendered SVG, and a rasterized PNG. Rendering goes through the one engine boundary so nothing else
// touches Mermaid; the produced bytes are handed to the save-file IPC by the toolbar. The PNG path
// draws the SVG onto a canvas, so it is a real-window concern (headless renderers have no raster
// canvas) — copy-source and the source/SVG text paths do not depend on rendering.

import { MERMAID_PNG_SCALE } from "./const";
import { renderDiagram } from "./engine";

/** The three shapes a diagram can be saved as: rendered vector, raw source, rasterized bitmap. */
export type DiagramExportFormat = "svg" | "mmd" | "png";

/** The file extension and save-dialog filter label for each export format. */
export const DIAGRAM_EXPORT_FILE: Record<
  DiagramExportFormat,
  { extension: string; label: string }
> = {
  svg: { extension: "svg", label: "SVG image" },
  mmd: { extension: "mmd", label: "Mermaid source" },
  png: { extension: "png", label: "PNG image" },
};

const encoder = new TextEncoder();

/** Render `source` to sanitized SVG markup, throwing the parse error so a caller can surface it. */
export async function renderSvg(source: string): Promise<string> {
  const result = await renderDiagram(source);
  if ("error" in result) throw new Error(result.error);
  return result.svg;
}

/** Copy the raw Mermaid source to the clipboard. */
export function copyDiagramSource(source: string): Promise<void> {
  return navigator.clipboard?.writeText(source) ?? Promise.resolve();
}

/** Copy the rendered SVG markup to the clipboard as text. */
export async function copyDiagramSvg(source: string): Promise<void> {
  await navigator.clipboard?.writeText(await renderSvg(source));
}

/** The bytes for one export format — the rendered SVG, the raw source, or a rasterized PNG. */
export async function diagramExportBytes(
  source: string,
  format: DiagramExportFormat,
): Promise<Uint8Array> {
  if (format === "mmd") return encoder.encode(source);
  const svg = await renderSvg(source);
  if (format === "svg") return encoder.encode(svg);
  return rasterizeToPng(svg);
}

/** Draw an SVG string onto an offscreen canvas at {@link MERMAID_PNG_SCALE} and read back PNG bytes. */
async function rasterizeToPng(svg: string): Promise<Uint8Array> {
  const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }));
  try {
    const image = await loadImage(url);
    const width = Math.max(1, Math.round((image.naturalWidth || image.width) * MERMAID_PNG_SCALE));
    const height = Math.max(
      1,
      Math.round((image.naturalHeight || image.height) * MERMAID_PNG_SCALE),
    );
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Could not rasterize diagram.");
    context.drawImage(image, 0, 0, width, height);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!blob) throw new Error("Could not rasterize diagram.");
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Could not rasterize diagram."));
    image.src = src;
  });
}
