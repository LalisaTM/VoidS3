import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { desktop, message } from "./api";

export type PaneSide = "local" | "remote";
export interface DropTarget {
  side: PaneSide;
  path: string;
}
interface DragPayload {
  side: PaneSide;
  paths: string[];
  x: number;
  y: number;
}

export function dropTargetAt(x: number, y: number): DropTarget | undefined {
  const element = document.elementFromPoint(x, y);
  const pane = element?.closest<HTMLElement>("[data-drop-side]");
  const side = pane?.dataset.dropSide;
  if (!pane || (side !== "local" && side !== "remote")) return;
  return {
    side,
    path:
      element?.closest<HTMLElement>("[data-entry-folder]")?.dataset
        .entryFolder ??
      pane.dataset.dropPath ??
      "",
  };
}

export function useFileDragDrop(options: {
  canUpload: boolean;
  canDownload: boolean;
  onSelect: (side: PaneSide, paths: string[]) => void;
  onTransfer: (source: PaneSide, paths: string[], target: DropTarget) => void;
  onExternalDrop: (paths: string[], target?: DropTarget) => void;
  onError: (message: string) => void;
}) {
  const latest = useRef(options);
  useEffect(() => {
    latest.current = options;
  });
  const pending = useRef<
    | (DragPayload & {
        pointerId: number;
        element: HTMLElement;
        started: boolean;
      })
    | undefined
  >(undefined);
  const ghost = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<DragPayload>();
  const [target, setTarget] = useState<DropTarget>();
  const showTarget = useCallback((next?: DropTarget) => {
    setTarget((old) =>
      old?.side === next?.side && old?.path === next?.path ? old : next,
    );
  }, []);
  function begin(
    event: ReactPointerEvent<HTMLElement>,
    side: PaneSide,
    selection: string[],
  ) {
    if (event.button !== 0 || event.pointerType === "touch") return;
    const element = event.target as Element;
    if (element.closest("button,input,select,[role=menuitem]")) return;
    const path =
      element.closest<HTMLElement>("[data-entry-path]")?.dataset.entryPath;
    if (!path) return;
    pending.current = {
      side,
      paths: selection.includes(path) ? selection : [path],
      x: event.clientX,
      y: event.clientY,
      pointerId: event.pointerId,
      element: event.currentTarget,
      started: false,
    };
  }
  useEffect(() => {
    function validTarget(drag: DragPayload, point: DropTarget | undefined) {
      if (!point || point.side === drag.side) return;
      if (
        drag.side === "local"
          ? !latest.current.canUpload
          : !latest.current.canDownload || !point.path
      )
        return;
      return point;
    }
    function finish() {
      const drag = pending.current;
      if (drag?.element.hasPointerCapture?.(drag.pointerId))
        drag.element.releasePointerCapture(drag.pointerId);
      pending.current = undefined;
      setDragging(undefined);
      showTarget();
    }
    function move(event: PointerEvent) {
      const drag = pending.current;
      if (!drag || event.pointerId !== drag.pointerId) return;
      if (!drag.started) {
        if (Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < 7)
          return;
        drag.started = true;
        drag.element.setPointerCapture?.(event.pointerId);
        latest.current.onSelect(drag.side, drag.paths);
        setDragging({ ...drag, x: event.clientX, y: event.clientY });
      }
      event.preventDefault();
      if (ghost.current)
        ghost.current.style.transform = `translate3d(${event.clientX + 14}px,${event.clientY + 14}px,0)`;
      showTarget(validTarget(drag, dropTargetAt(event.clientX, event.clientY)));
    }
    function up(event: PointerEvent) {
      const drag = pending.current;
      if (!drag || event.pointerId !== drag.pointerId) return;
      const hit = drag.started
        ? dropTargetAt(event.clientX, event.clientY)
        : undefined;
      const point = validTarget(drag, hit);
      finish();
      if (point) latest.current.onTransfer(drag.side, drag.paths, point);
      else if (drag.started && hit && hit.side !== drag.side)
        latest.current.onError(
          drag.side === "local"
            ? "Connect a writable bucket before uploading dropped files."
            : "Choose a local destination folder before downloading dropped files.",
        );
    }
    function escape(event: KeyboardEvent) {
      if (event.key === "Escape") finish();
    }
    window.addEventListener("pointermove", move, { passive: false });
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", finish);
    window.addEventListener("blur", finish);
    window.addEventListener("keydown", escape);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", finish);
      window.removeEventListener("blur", finish);
      window.removeEventListener("keydown", escape);
    };
  }, [showTarget]);
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "leave") {
          showTarget();
          return;
        }
        const scale = window.devicePixelRatio || 1;
        const point = dropTargetAt(
          event.payload.position.x / scale,
          event.payload.position.y / scale,
        );
        if (event.payload.type === "drop") {
          showTarget();
          latest.current.onExternalDrop(event.payload.paths, point);
        } else {
          showTarget(
            point?.side === "remote" && latest.current.canUpload
              ? point
              : undefined,
          );
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error: unknown) => latest.current.onError(message(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [showTarget]);
  return { begin, dragging, target, ghost };
}
