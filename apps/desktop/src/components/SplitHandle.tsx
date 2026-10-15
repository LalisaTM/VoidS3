import { useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

export function SplitHandle() {
  const handle = useRef<HTMLDivElement>(null);
  const [share, setShare] = useState(50);
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    handle.current?.parentElement?.style.setProperty("--local-share", "50%");
  }, []);
  function resize(value: number) {
    const parent = handle.current?.parentElement;
    if (!parent) return;
    const min = Math.max(30, (280 / Math.max(1, parent.clientWidth - 5)) * 100);
    const next = Math.min(100 - min, Math.max(min, value));
    parent.style.setProperty("--local-share", `${next}%`);
    setShare(Math.round(next));
  }
  return (
    <div
      ref={handle}
      className="pane-divider"
      role="separator"
      aria-label="Resize file panes"
      aria-orientation="vertical"
      aria-valuemin={30}
      aria-valuemax={70}
      aria-valuenow={share}
      tabIndex={0}
      data-dragging={dragging || undefined}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(true);
      }}
      onPointerMove={(e) => {
        if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
        const rect = e.currentTarget.parentElement!.getBoundingClientRect();
        resize(((e.clientX - rect.left) / rect.width) * 100);
      }}
      onPointerUp={() => setDragging(false)}
      onPointerCancel={() => setDragging(false)}
      onLostPointerCapture={() => setDragging(false)}
      onKeyDown={(e) => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
        e.preventDefault();
        resize(
          e.key === "Home"
            ? 30
            : e.key === "End"
              ? 70
              : share + (e.key === "ArrowLeft" ? -5 : 5),
        );
      }}
    >
      <span aria-hidden="true">
        <ChevronLeft size={10} />
        <ChevronRight size={10} />
      </span>
    </div>
  );
}
