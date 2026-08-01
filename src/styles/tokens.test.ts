import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/** Read a stylesheet relative to this test file so vitest CSS stubs cannot interfere. */
function readCss(relativePath: string): string {
  return readFileSync(
    fileURLToPath(new URL(relativePath, import.meta.url)),
    "utf8",
  );
}

const tokensCss = readCss("./tokens.css");
const appCss = readCss("../App.css");
const fileListCss = readCss("../components/FileList/FileList.css");
const folderTreeCss = readCss("../components/FolderTree/FolderTree.css");
const playerTransportCss = readCss(
  "../components/PlayerTransport/PlayerTransport.css",
);
const confirmDialogCss = readCss(
  "../components/ConfirmDialog/ConfirmDialog.css",
);

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

function definedTokens(css: string): Set<string> {
  const defined = new Set<string>();
  const pattern = /(?:^|\s)--([a-zA-Z0-9-]+)\s*:/g;
  for (const match of css.matchAll(pattern)) {
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
  const defined = definedTokens(tokensCss);

  it("defines every required semantic token", () => {
    const missing = requiredTokens.filter((token) => !defined.has(token));
    expect(missing).toEqual([]);
  });

  it("resolves every var() reference used by feature styles", () => {
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
