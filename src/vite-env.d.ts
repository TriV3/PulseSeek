/// <reference types="vite/client" />

/** Injected by Vite from `src-tauri/Cargo.toml`; see `src/version.ts`. */
declare const __PULSESEEK_VERSION__: string;

interface ImportMetaEnv {
  readonly TAURI_ENV_PLATFORM?: string;
}
