/**
 * Application version injected by Vite at build time from the Rust package
 * manifest (`src-tauri/Cargo.toml`), which remains the single source of truth.
 *
 * The value is replaced through the `__PULSESEEK_VERSION__` define in
 * `vite.config.ts`; the declaration lives in `vite-env.d.ts`.
 */
export const APP_VERSION: string = __PULSESEEK_VERSION__;
