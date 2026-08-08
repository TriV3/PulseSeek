import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import "./ContextMenu.css";

interface ContextMenuProps {
  label: string;
  x: number;
  y: number;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
  children: ReactNode;
}

/** Accessible application context menu rendered outside clipped panels. */
export function ContextMenu({
  label,
  x,
  y,
  onClose,
  returnFocus = null,
  children,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ x, y });

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    const margin = 4;
    setPosition({
      x: Math.max(
        margin,
        Math.min(x, window.innerWidth - menu.offsetWidth - margin),
      ),
      y: Math.max(
        margin,
        Math.min(y, window.innerHeight - menu.offsetHeight - margin),
      ),
    });
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [x, y]);

  useEffect(() => {
    const closeFromOutside = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose();
    };
    window.addEventListener("pointerdown", closeFromOutside, true);
    window.addEventListener("blur", onClose);
    return () => {
      window.removeEventListener("pointerdown", closeFromOutside, true);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);

  const closeAndRestoreFocus = () => {
    onClose();
    window.setTimeout(() => returnFocus?.focus(), 0);
  };

  const moveFocus = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ) ?? [],
    );
    if (event.key === "Escape") {
      event.preventDefault();
      closeAndRestoreFocus();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : event.key === "ArrowDown"
            ? (current + 1) % items.length
            : (current - 1 + items.length) % items.length;
    items[next]?.focus();
  };

  return createPortal(
    <div
      ref={menuRef}
      className="context-menu"
      role="menu"
      aria-label={label}
      style={{ left: position.x, top: position.y }}
      onKeyDown={moveFocus}
      onClick={(event) => {
        const item = (event.target as HTMLElement).closest("button");
        if (item && !item.disabled) onClose();
      }}
    >
      {children}
    </div>,
    document.body,
  );
}
