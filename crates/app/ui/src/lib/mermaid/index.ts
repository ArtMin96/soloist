// The public surface of the Mermaid module: rendering, theming, and the shared constants. Consumers
// import from here; only the diagram component reaches for the engine submodule directly (so a test can
// mock the library boundary in isolation).

export {
  MERMAID_ID_PREFIX,
  MERMAID_LANGUAGE,
  MERMAID_SECURITY_LEVEL,
  MERMAID_FONT_SIZE,
  MERMAID_RENDER_DEBOUNCE_MS,
  MERMAID_STARTER_SOURCE,
  MERMAID_THEME_TOKENS,
  MERMAID_DEFAULT_ZOOM,
  MIN_MERMAID_ZOOM,
  MAX_MERMAID_ZOOM,
  MERMAID_ZOOM_STEP,
  MERMAID_PNG_SCALE,
} from "./const";
export { renderDiagram, parseDiagram, type RenderResult, type ParseResult } from "./engine";
export { isDarkTheme, themeSignature, mermaidThemeConfig, type MermaidThemeConfig } from "./theme";
export { useMermaidTheme } from "./useMermaidTheme";
export { clampZoom, zoomAround, IDENTITY_TRANSFORM, type Transform } from "./zoom";
export {
  readDiagramTheme,
  setDiagramTheme,
  DIAGRAM_THEME_VALUES,
  DIAGRAM_THEME_LABELS,
  type DiagramTheme,
} from "./frontmatter";
export {
  renderSvg,
  copyDiagramSource,
  copyDiagramSvg,
  diagramExportBytes,
  DIAGRAM_EXPORT_FILE,
  type DiagramExportFormat,
} from "./export";
