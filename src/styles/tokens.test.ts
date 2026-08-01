import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/** Read a stylesheet relative to this test file so vitest CSS stubs cannot interfere. */
function readCss(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    "utf8",
  );
}

const appCss = readCss("../App.css");
const fileListCss = readCss("../components/FileList/FileList.css");
const folderTreeCss = readCss("../components/FolderTree/FolderTree.css");
const playerTransportCss = readCss(
  "../components/PlayerTransport/PlayerTransport.css",
);
const confirmDialogCss = readCss(
  "../components/ConfirmDialog/ConfirmDialog.css",
);

// Vite rewrites `new URL(<literal>, import.meta.url)` into an http:// dev
// URL, so resolve through a variable to keep a file: URL for readdirSync.
const themesRelative = "./themes";
const themeFiles = readdirSync(new URL(themesRelative, import.meta.url))
  .filter((name) => name.endsWith(".css"))
  .sort()
  .map((name) => ({ name, css: readCss(`./themes/${name}`) }));

/** The semantic tokens every PulseSeek theme must define. */
const requiredTokens = [
  "bg-canvas",
  "bg-surface",
  "bg-subtle",
  "bg-panel",
  "bg-chrome",
  "bg-chrome-strong",
  "bg-hover",
  "bg-selected",
  "bg-header",
  "bg-header-strong",
  "bg-button",
  "bg-button-strong",
  "bg-browser-tabs",
  "text",
  "text-strong",
  "text-muted",
  "text-subtle",
  "text-brand",
  "text-danger",
  "text-on-accent",
  "line",
  "line-strong",
  "line-soft",
  "accent",
  "accent-strong",
  "accent-play",
  "accent-play-soft",
  "accent-play-ink",
  "accent-play-border",
  "accent-range",
  "bg-danger",
  "bg-danger-strong",
  "bg-danger-subtle",
  "line-danger",
  "line-danger-strong",
  "wave",
  "wave-grid",
  "wave-selection",
  "wave-selection-border",
  "wave-playhead",
  "focus-ring",
  "overlay-backdrop",
  "shadow-overlay",
  "spinner-track",
  "spinner-head",
];

/** Stylesheets that must render through semantic tokens only. */
const featureStyles = [
  appCss,
  fileListCss,
  folderTreeCss,
  playerTransportCss,
  confirmDialogCss,
];

const rawColorPattern =
  /(?:#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|\bcolor\(|transparent)/;

/** Tokens declared in the first CSS block of a theme file. */
function tokensInTheme(css: string): Set<string> {
  const block = css.match(/\{([^}]*)\}/)?.[1] ?? "";
  const defined = new Set<string>();
  const pattern = /--([a-zA-Z0-9-]+)\s*:/g;
  for (const match of block.matchAll(pattern)) {
    defined.add(match[1]);
  }
  return defined;
}

function referencedTokens(css: string): Set<string> {
  const referenced = new Set<string>();
  const pattern = /var\(\s*--([a-zA-Z0-9-]+)/g;
  for (const match of css.matchAll(pattern)) {
    referenced.add(match[1]);
  }
  return referenced;
}

describe("semantic design tokens", () => {
  it("defines every required semantic token in each theme file", () => {
    for (const theme of themeFiles) {
      const defined = tokensInTheme(theme.css);
      const missing = requiredTokens.filter((token) => !defined.has(token));
      expect(missing, `${theme.name} is missing required tokens`).toEqual([]);
    }
  });

  it("discovers at least the light and dark themes", () => {
    expect(themeFiles.map((theme) => theme.name)).toEqual(
      expect.arrayContaining(["dark.css", "light.css"]),
    );
  });

  it("resolves every var() reference used by feature styles", () => {
    const defined = new Set<string>();
    for (const theme of themeFiles) {
      for (const token of tokensInTheme(theme.css)) {
        defined.add(token);
      }
    }

    const referenced = new Set<string>();
    for (const css of featureStyles) {
      for (const token of referencedTokens(css)) {
        referenced.add(token);
      }
    }
    const dangling = [...referenced].filter((token) => !defined.has(token));
    expect(dangling).toEqual([]);
  });

  it("leaves no palette colors in feature stylesheets", () => {
    for (const css of featureStyles) {
      const matches = css.match(rawColorPattern);
      expect(matches, "raw color found in feature stylesheet").toBeNull();
    }
  });
});
