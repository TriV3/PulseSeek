# Compact player controls

The now-playing header exposes `Options` beside the compact-mode toggle in
both normal and compact layouts. These controls remain available when compact
mode hides the sidebar, A-B controls, and visualization selectors.

`Options` contains the single playback-mode selector used by both layouts. It
offers One shot, Loop current, Sequential, and Random through the existing
typed playback command and persisted player preference. Crossfade is not a
playback mode; gapless continuation remains a separate Sequential preference.

The options panel closes after an outside pointer action or Escape. Escape
returns keyboard focus to the Options trigger. The panel uses viewport-relative
positioning and bounded scrolling so all settings remain reachable in the
minimum compact window.
