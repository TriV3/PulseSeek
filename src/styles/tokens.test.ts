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
const logAnalyzerCss = readCss(
  "../components/LogAnalyzer/LogAnalyzerCanvas.css",
);
const linearAnalyzerCss = readCss(
  "../components/LinearAnalyzer/LinearAnalyzerCanvas.css",
);
const musicalSpectrumCss = readCss(
  "../components/MusicalSpectrum/MusicalSpectrumCanvas.css",
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
  "folder-active-path",
  "folder-bookmark",
  "browser-icon-computer",
  "browser-icon-system",
  "browser-icon-home",
  "browser-icon-physical",
  "browser-icon-network",
  "browser-icon-folder",
  "browser-icon-library",
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
  "wave-soft",
  "wave-selection",
  "wave-selection-border",
  "wave-seek-current",
  "wave-seek-hover",
  "wave-time-current",
  "wave-time-hover",
  "analyzer-spectrum",
  "analyzer-spectrum-soft",
  "analyzer-grid",
  "analyzer-label",
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
  logAnalyzerCss,
  linearAnalyzerCss,
  musicalSpectrumCss,
];

/** Component-local properties written imperatively at runtime, not theme tokens. */
const runtimeProperties = new Set(["seek-x"]);

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
  it("moves seek overlays through compositor transforms instead of layout", () => {
    expect(appCss).toMatch(
      /\.waveform-current-marker\s*\{[^}]*transform:\s*translate3d/s,
    );
    expect(appCss).toMatch(
      /\.waveform-hover-marker\s*\{[^}]*transform:\s*translate3d/s,
    );
    expect(appCss).toMatch(
      /\.waveform-time\s*\{[^}]*transform:\s*translate3d/s,
    );
    expect(appCss).not.toMatch(/transition:\s*left/);
  });

  it.each(themeFiles)(
    "$name distinguishes current and hover seek indicators",
    ({ css }) => {
      expect(tokenValue(css, "wave-seek-current")).not.toBe(
        tokenValue(css, "wave-seek-hover"),
      );
      expect(tokenValue(css, "wave-time-current")).not.toBe(
        tokenValue(css, "wave-time-hover"),
      );
    },
  );

  it("gives the hover seek indicator a distinct color in every theme", () => {
    const colors = themeFiles.map((theme) =>
      tokenValue(theme.css, "wave-seek-hover"),
    );
    expect(new Set(colors).size).toBe(themeFiles.length);
  });
  it("defines every required semantic token in each theme file", () => {
    for (const theme of themeFiles) {
      const defined = tokensInTheme(theme.css);
      const missing = requiredTokens.filter((token) => !defined.has(token));
      expect(missing, `${theme.name} is missing required tokens`).toEqual([]);
    }
  });

  it("discovers at least the light, dark, midnight, and high-contrast themes", () => {
    expect(themeFiles.map((theme) => theme.name)).toEqual(
      expect.arrayContaining([
        "dark.css",
        "light.css",
        "midnight.css",
        "high-contrast.css",
      ]),
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
    const dangling = [...referenced].filter(
      (token) => !defined.has(token) && !runtimeProperties.has(token),
    );
    expect(dangling).toEqual([]);
  });

  it("leaves no palette colors in feature stylesheets", () => {
    for (const css of featureStyles) {
      const matches = css.match(rawColorPattern);
      expect(matches, "raw color found in feature stylesheet").toBeNull();
    }
  });
});

// ── WCAG contrast helpers ──────────────────────────────────────────────

interface Rgb {
  r: number;
  g: number;
  b: number;
}

function channelToLinear(channel: number): number {
  const value = channel / 255;
  return value <= 0.03928
    ? value / 12.92
    : Math.pow((value + 0.055) / 1.055, 2.4);
}

function relativeLuminance({ r, g, b }: Rgb): number {
  return (
    0.2126 * channelToLinear(r) +
    0.7152 * channelToLinear(g) +
    0.0722 * channelToLinear(b)
  );
}

/** WCAG 2.x contrast ratio between two solid colors. */
function contrastRatio(a: Rgb, b: Rgb): number {
  const [lighter, darker] = [relativeLuminance(a), relativeLuminance(b)].sort(
    (x, y) => y - x,
  );
  return (lighter + 0.05) / (darker + 0.05);
}

function hexToRgb(hex: string): Rgb | null {
  const match = hex.match(/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/);
  if (!match) return null;
  let value = match[1];
  if (value.length === 3) {
    value = value
      .split("")
      .map((channel) => channel + channel)
      .join("");
  }
  const number = parseInt(value, 16);
  return { r: (number >> 16) & 255, g: (number >> 8) & 255, b: number & 255 };
}

function tokenValue(css: string, token: string): string | null {
  const block = css.match(/\{([^}]*)\}/)?.[1] ?? "";
  const pattern = new RegExp(`--${token}\\s*:\\s*([^;]+);`);
  return block.match(pattern)?.[1]?.trim() ?? null;
}

function assertContrast(
  theme: { name: string; css: string },
  foreground: string,
  background: string,
  minimum: number,
): void {
  const foregroundColor = tokenValue(theme.css, foreground);
  const backgroundColor = tokenValue(theme.css, background);
  const foregroundRgb = foregroundColor ? hexToRgb(foregroundColor) : null;
  const backgroundRgb = backgroundColor ? hexToRgb(backgroundColor) : null;
  expect(
    foregroundRgb,
    `${theme.name}: ${foreground} must be a solid hex color`,
  ).not.toBeNull();
  expect(
    backgroundRgb,
    `${theme.name}: ${background} must be a solid hex color`,
  ).not.toBeNull();
  const ratio = contrastRatio(foregroundRgb!, backgroundRgb!);
  expect(
    ratio,
    `${theme.name}: ${foreground} vs ${background}`,
  ).toBeGreaterThanOrEqual(minimum);
}

describe("theme accessibility", () => {
  const highContrast = themeFiles.find(
    (theme) => theme.name === "high-contrast.css",
  );

  it("keeps primary text at AA contrast on every theme", () => {
    for (const theme of themeFiles) {
      assertContrast(theme, "text", "bg-canvas", 4.5);
    }
  });

  it("keeps the active audio path at AA contrast in every browser state", () => {
    for (const theme of themeFiles) {
      assertContrast(theme, "folder-active-path", "bg-subtle", 4.5);
      assertContrast(theme, "folder-active-path", "bg-hover", 4.5);
      assertContrast(theme, "folder-active-path", "bg-selected", 4.5);
    }
    expect(folderTreeCss).toContain("color: var(--folder-active-path)");
  });

  it("keeps bookmarked folders at AA contrast in every browser state", () => {
    for (const theme of themeFiles) {
      assertContrast(theme, "folder-bookmark", "bg-subtle", 4.5);
      assertContrast(theme, "folder-bookmark", "bg-hover", 4.5);
      assertContrast(theme, "folder-bookmark", "bg-selected", 4.5);
    }
    expect(folderTreeCss).toContain("color: var(--folder-bookmark)");
    expect(folderTreeCss).toMatch(
      /\.go-up-btn\s*\{[^}]*color:\s*var\(--text-strong\)/s,
    );
  });

  it("keeps High Contrast primary text at AAA contrast", () => {
    const theme = highContrast;
    expect(theme, "high-contrast.css is missing").toBeDefined();
    assertContrast(theme!, "text", "bg-canvas", 7);
    assertContrast(theme!, "text-strong", "bg-canvas", 7);
    assertContrast(theme!, "text-muted", "bg-canvas", 7);
  });

  it("keeps High Contrast controls and focus visible", () => {
    const theme = highContrast;
    expect(theme, "high-contrast.css is missing").toBeDefined();
    assertContrast(theme!, "text-on-accent", "accent", 4.5);
    assertContrast(theme!, "focus-ring", "bg-canvas", 3);
    assertContrast(theme!, "accent", "bg-surface", 3);
  });
});
