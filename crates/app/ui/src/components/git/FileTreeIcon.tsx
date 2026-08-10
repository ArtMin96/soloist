import {
  FileArchiveIcon,
  FileCogIcon,
  FileImageIcon,
  FileKeyIcon,
  FileSpreadsheetIcon,
  FileTerminalIcon,
  FileTextIcon,
  FolderIcon,
  FolderOpenIcon,
  type LucideIcon,
} from "lucide-react";
import {
  DiCss3,
  DiGo,
  DiHtml5,
  DiJava,
  DiJavascript1,
  DiMarkdown,
  DiPhp,
  DiPython,
  DiRuby,
  DiRust,
  DiSass,
  DiSwift,
} from "react-icons/di";
import {
  SiC,
  SiCplusplus,
  SiDart,
  SiElixir,
  SiHaskell,
  SiJson,
  SiKotlin,
  SiLua,
  SiR,
  SiReact,
  SiScala,
  SiSharp,
  SiSvelte,
  SiTypescript,
  SiVuedotjs,
  SiYaml,
} from "react-icons/si";
import type { IconType } from "react-icons";
import { cn } from "@/lib/utils";
import type { TreeNode } from "@/store/git/tree";

interface FileTreeIconProps {
  node: TreeNode;
  expanded: boolean;
}

/**
 * The visual vocabulary for repository paths. It deliberately stays presentational: file names
 * remain the tree's accessible label, while the small, familiar IDE glyphs make a dense tree
 * easier to scan without introducing another interaction or state model.
 */
export function FileTreeIcon({ node, expanded }: FileTreeIconProps) {
  if (node.folder) {
    const Folder = expanded ? FolderOpenIcon : FolderIcon;
    return <Folder aria-hidden className="size-4.5 shrink-0 text-primary/80" />;
  }

  const { Icon, className } = iconFor(node.name);
  return <Icon aria-hidden className={cn("size-4 shrink-0", className)} />;
}

function iconFor(name: string): { Icon: LucideIcon | IconType; className: string } {
  const normalized = name.toLowerCase();
  const extension = normalized.includes(".")
    ? normalized.slice(normalized.lastIndexOf(".") + 1)
    : "";

  const language = LANGUAGE_ICONS[extension];
  if (language) return language;

  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "avif"].includes(extension)) {
    return { Icon: FileImageIcon, className: "text-muted-foreground" };
  }
  if (["zip", "tar", "gz", "bz2", "xz", "7z", "rar"].includes(extension)) {
    return { Icon: FileArchiveIcon, className: "text-muted-foreground" };
  }
  if (["csv", "tsv", "xlsx", "ods"].includes(extension)) {
    return { Icon: FileSpreadsheetIcon, className: "text-muted-foreground" };
  }
  if (["sh", "bash", "zsh", "fish", "ps1", "bat", "cmd"].includes(extension)) {
    return { Icon: FileTerminalIcon, className: "text-muted-foreground" };
  }
  if (["env", "pem", "key", "crt", "cer"].includes(extension) || normalized.startsWith(".env")) {
    return { Icon: FileKeyIcon, className: "text-muted-foreground" };
  }
  if (
    ["md", "mdx", "txt", "rst", "adoc"].includes(extension) ||
    ["readme", "license", "notice"].includes(normalized)
  ) {
    return { Icon: FileTextIcon, className: "text-muted-foreground" };
  }
  if (["lock", "lockb"].includes(extension) || normalized.endsWith(".lock")) {
    return { Icon: FileCogIcon, className: "text-muted-foreground" };
  }

  return { Icon: FileTextIcon, className: "text-muted-foreground" };
}

/** Recognizable, language-specific marks make mixed repositories scannable at a glance. */
const LANGUAGE_ICONS: Record<string, { Icon: IconType; className: string }> = {
  c: { Icon: SiC, className: "text-file-language-blue" },
  cc: { Icon: SiCplusplus, className: "text-file-language-blue" },
  cpp: { Icon: SiCplusplus, className: "text-file-language-blue" },
  cs: { Icon: SiSharp, className: "text-file-language-violet" },
  css: { Icon: DiCss3, className: "text-file-language-azure" },
  cxx: { Icon: SiCplusplus, className: "text-file-language-blue" },
  dart: { Icon: SiDart, className: "text-file-language-cyan" },
  elixir: { Icon: SiElixir, className: "text-file-language-violet" },
  ex: { Icon: SiElixir, className: "text-file-language-violet" },
  exs: { Icon: SiElixir, className: "text-file-language-violet" },
  go: { Icon: DiGo, className: "text-file-language-cyan" },
  h: { Icon: SiC, className: "text-file-language-blue" },
  hpp: { Icon: SiCplusplus, className: "text-file-language-blue" },
  hs: { Icon: SiHaskell, className: "text-file-language-violet" },
  html: { Icon: DiHtml5, className: "text-file-language-orange" },
  htm: { Icon: DiHtml5, className: "text-file-language-orange" },
  java: { Icon: DiJava, className: "text-file-language-red" },
  js: { Icon: DiJavascript1, className: "text-file-language-amber" },
  jsx: { Icon: SiReact, className: "text-file-language-cyan" },
  json: { Icon: SiJson, className: "text-file-language-amber" },
  jsonc: { Icon: SiJson, className: "text-file-language-amber" },
  kt: { Icon: SiKotlin, className: "text-file-language-violet" },
  kts: { Icon: SiKotlin, className: "text-file-language-violet" },
  lua: { Icon: SiLua, className: "text-file-language-blue" },
  md: { Icon: DiMarkdown, className: "text-file-language-azure" },
  mdx: { Icon: DiMarkdown, className: "text-file-language-azure" },
  php: { Icon: DiPhp, className: "text-file-language-violet" },
  py: { Icon: DiPython, className: "text-file-language-azure" },
  r: { Icon: SiR, className: "text-file-language-blue" },
  rb: { Icon: DiRuby, className: "text-file-language-red" },
  rs: { Icon: DiRust, className: "text-file-language-orange" },
  sass: { Icon: DiSass, className: "text-file-language-pink" },
  scala: { Icon: SiScala, className: "text-file-language-red" },
  scss: { Icon: DiSass, className: "text-file-language-pink" },
  svelte: { Icon: SiSvelte, className: "text-file-language-orange" },
  swift: { Icon: DiSwift, className: "text-file-language-orange" },
  ts: { Icon: SiTypescript, className: "text-file-language-azure" },
  tsx: { Icon: SiReact, className: "text-file-language-cyan" },
  vue: { Icon: SiVuedotjs, className: "text-file-language-green" },
  yaml: { Icon: SiYaml, className: "text-file-language-red" },
  yml: { Icon: SiYaml, className: "text-file-language-red" },
};
