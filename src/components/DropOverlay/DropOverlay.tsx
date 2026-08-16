/** Full-window feedback shown while external files hover over the window
 * (FR-DI-001). Purely decorative: drag-and-drop is pointer-driven, so the
 * overlay never receives focus or pointer events. */
import "./DropOverlay.css";

export function DropOverlay({ active }: { active: boolean }) {
  if (!active) return null;
  return (
    <div className="drop-overlay" aria-hidden="true">
      <div className="drop-overlay-badge">
        <span className="drop-overlay-icon" aria-hidden="true">
          ⬇
        </span>
        <span>Drop files to play or reveal</span>
      </div>
    </div>
  );
}
