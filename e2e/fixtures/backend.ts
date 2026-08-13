import { test as base } from "@playwright/test";

// ── Browser-side mock backend ─────────────────────────────────────────────

const SCRIPT = `
(function() {
  if (window.__TAURI_BACKEND__) return;
  var defaultShortcuts = [
    ["open_folder", "o", true, false, false],
    ["toggle_play_pause", "space", false, false, false],
    ["play_selection", "enter", false, false, false],
    ["previous_track", "arrowup", false, false, false],
    ["next_track", "arrowdown", false, false, false],
    ["seek_backward", "arrowleft", false, false, false],
    ["seek_forward", "arrowright", false, false, false],
    ["toggle_loop", "l", false, false, false],
    ["move_to_trash", "delete", false, false, false],
    ["refresh", "r", true, false, false],
    ["focus_search", "f", true, false, false],
    ["set_playback_mode_one_shot", "1", true, false, true],
    ["set_playback_mode_loop_current", "2", true, false, true],
    ["set_playback_mode_sequential", "3", true, false, true],
    ["set_playback_mode_random", "4", true, false, true],
    ["mark_keep", "k", true, true, false],
    ["mark_maybe", "m", true, true, false],
    ["mark_reject", "r", true, true, false],
    ["mark_favorite", "f", true, true, false],
    ["mark_clear", "u", true, true, false],
    ["set_ab_start", "[", false, false, false],
    ["set_ab_end", "]", false, false, false],
    ["toggle_ab_repeat", "a", false, false, false]
  ].map(function(binding) {
    return { action_id: binding[0], key: binding[1], primary: binding[2], shift: binding[3], alt: binding[4] };
  });
  var unavailableShortcuts = [];
  var state = {
    shortcuts: defaultShortcuts.map(function(mapping) { return Object.assign({}, mapping); }),
    visualizationSettings: { enabled: true, mode: "waveform", quality: "balanced" },
    commandHandlers: {
      list_browser_roots: function() { return { roots: [{ path: "/home/test", name: "Home", kind: "home" }, { path: "/music", name: "Music", kind: "physical" }], libraries: [{ path: "/downloads", name: "Downloads", kind: "downloads" }] }; },
      list_folder_bookmarks: function() { return { bookmarks: [] }; },
      add_folder_bookmark: function() { return {}; },
      remove_folder_bookmark: function() { return {}; },
      list_devices: function() { return { devices: [] }; },
      current_device: function() { return { device: null }; },
      set_playback_mode: function(args) { return { mode: args.mode }; },
      seek: function(args) { return { position_ms: args.position_ms }; },
      volume: function() { return {}; },
    load_player_preferences: function() { return { version: 1, preferences: { schema_version: 1, revision: 0, playback_mode: "one-shot", output_device_id: null, volume: 1, muted: false, waveform_size: 38, browser_size: 24, selected_folder_path: null, expanded_folder_paths: [], last_played_file_path: null, last_played_position_ms: 0, last_played_duration_ms: null, theme: "system", waveform_style: "outline", seek_step_mode: "auto", show_hidden_folders: false } }; },
      save_player_preferences: function(args) { return { version: 1, preferences: args.preferences }; },
      load_visualization_settings: function() { return { version: 1, settings: Object.assign({}, state.visualizationSettings) }; },
      save_visualization_settings: function(args) { state.visualizationSettings = Object.assign({}, args.settings); return { version: 1, settings: Object.assign({}, state.visualizationSettings) }; },
      load_shortcuts: function() { return { mappings: state.shortcuts, unavailable_action_ids: unavailableShortcuts }; },
      save_shortcuts: function(args) { state.shortcuts = args.mappings; return { mappings: state.shortcuts, unavailable_action_ids: unavailableShortcuts }; },
      reset_shortcuts: function() { state.shortcuts = defaultShortcuts.map(function(mapping) { return Object.assign({}, mapping); }); return { mappings: state.shortcuts, unavailable_action_ids: unavailableShortcuts }; },
      get_waveform: function() {
        var n = 96;
        var min = [];
        var max = [];
        for (var i = 0; i < n; i++) {
          var v = Math.sin(i / 6) * 0.6;
          min.push(v - 0.2);
          max.push(v + 0.2);
        }
        return { format_version: 1, channels: 1, samples_per_peak: 64, min: min, max: max };
      }
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
