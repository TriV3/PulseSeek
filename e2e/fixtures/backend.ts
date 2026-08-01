import { test as base } from "@playwright/test";

// ── Browser-side mock backend ─────────────────────────────────────────────

const SCRIPT = `
(function() {
  if (window.__TAURI_BACKEND__) return;
  var state = {
    commandHandlers: {
      list_browser_roots: function() { return { roots: [{ path: "/music", name: "Music" }] }; },
      list_devices: function() { return { devices: [] }; },
      current_device: function() { return { device: null }; }
    },
    listeners: {},
    calls: []
  };
  window.__TAURI_BACKEND__ = {
    _state: state,
    mockCommand: function(cmd, resp) {
      state.commandHandlers[cmd] = function() { return resp; };
    },
    emit: function(evt, data) {
      var handlers = state.listeners[evt];
      if (handlers) {
        for (var i = 0; i < handlers.length; i++) {
          handlers[i]({ payload: data });
        }
      }
    },
    getCalls: function() { return state.calls; },
    // Called by the Tauri mock modules
    invoke: function(cmd, args) { state.calls.push({ command: cmd, payload: args }); var h = state.commandHandlers[cmd]; return h ? h(args) : undefined; },
    listen: function(evt, handler) { if (!state.listeners[evt]) state.listeners[evt] = []; state.listeners[evt].push(handler); return function() { var idx = state.listeners[evt].indexOf(handler); if (idx >= 0) state.listeners[evt].splice(idx, 1); }; }
  };
})();
`;

// ── Test fixtures ─────────────────────────────────────────────────────────

// Single auto fixture that injects the mock backend script once per context.
// Other fixtures depend on it via _backendSetup but don't call addInitScript.
const testFixtures = base.extend<{
  _backendSetup: void;
  mockCommand: (command: string, response: unknown) => Promise<void>;
  emitEvent: (event: string, payload: unknown) => Promise<void>;
  getCommandCalls: () => Promise<Array<{ command: string; payload: unknown }>>;
}>({
  _backendSetup: [
    async ({ context }, use) => {
      await context.addInitScript(SCRIPT);
      await use();
    },
    { auto: true },
  ],

  mockCommand: [
    async ({ page }, use) => {
      await use(async (command, response) => {
        await page.evaluate(
          `window.__TAURI_BACKEND__.mockCommand(${JSON.stringify(command)}, ${JSON.stringify(response)})`,
        );
      });
    },
    { auto: false },
  ],

  emitEvent: [
    async ({ page }, use) => {
      await use(async (event, payload) => {
        await page.evaluate(
          `window.__TAURI_BACKEND__.emit(${JSON.stringify(event)}, ${JSON.stringify(payload)})`,
        );
      });
    },
    { auto: false },
  ],

  getCommandCalls: [
    async ({ page }, use) => {
      await use(async () => {
        return await page.evaluate("window.__TAURI_BACKEND__.getCalls()");
      });
    },
    { auto: false },
  ],
});

export const test = testFixtures;
export { expect } from "@playwright/test";
