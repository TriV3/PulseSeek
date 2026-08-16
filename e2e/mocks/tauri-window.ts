// Mock for @tauri-apps/api/window — used during Playwright E2E tests.
// The real module requires window.__TAURI_INTERNALS__.metadata which does
// not exist in a plain browser. The compact-mode hook drives the real Tauri
// window; here it gets inert stubs that record nothing and never resize.

export class LogicalSize {
  width: number;
  height: number;
  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
}

export class PhysicalSize {
  width: number;
  height: number;
  constructor(width: number, height: number) {
    this.width = width;
    this.height = height;
  }
}

export function getCurrentWindow() {
  return {
    isMaximized: async () => false,
    innerSize: async () => ({ width: 1200, height: 800 }),
    scaleFactor: async () => 1,
    setSize: async () => {},
    setMinSize: async () => {},
    onResized: async () => () => {},
  };
}
