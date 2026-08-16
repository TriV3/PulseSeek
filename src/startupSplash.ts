const SPLASH_FADE_DURATION_MS = 220;
const SPLASH_VISIBLE_DURATION_MS = 1_500;

/** Dismisses the document-level splash after React has begun rendering. */
export function dismissStartupSplash(): void {
  const splash = document.querySelector<HTMLElement>("#startup-splash");
  if (!splash) return;

  window.setTimeout(() => {
    splash.classList.add("startup-splash--leaving");
    window.setTimeout(() => splash.remove(), SPLASH_FADE_DURATION_MS);
  }, SPLASH_VISIBLE_DURATION_MS);
}
