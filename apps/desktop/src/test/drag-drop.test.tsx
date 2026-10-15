import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useFileDragDrop } from "../lib/useFileDragDrop";
import type { DragDropEvent } from "@tauri-apps/api/webview";

const native = vi.hoisted(() => ({
  listener: undefined as
    undefined | ((event: { payload: DragDropEvent }) => void),
  stop: vi.fn(),
}));
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: true,
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: async (listener: typeof native.listener) => {
      native.listener = listener;
      return native.stop;
    },
  }),
}));
const onTransfer = vi.fn();
const onExternalDrop = vi.fn();
const onSelect = vi.fn();
const onError = vi.fn();
function Harness({ writable = true }: { writable?: boolean }) {
  const drag = useFileDragDrop({
    canUpload: writable,
    canDownload: true,
    onSelect,
    onTransfer,
    onExternalDrop,
    onError,
  });
  return (
    <>
      <section
        data-testid="local"
        data-drop-side="local"
        data-drop-path="C:/downloads"
        onPointerDown={(event) =>
          drag.begin(event, "local", ["C:/one.txt", "C:/two.txt"])
        }
      >
        <span data-entry-path="C:/one.txt">one.txt</span>
        <span data-entry-folder="C:/downloads/subfolder">local folder</span>
      </section>
      <section
        data-testid="remote"
        data-drop-side="remote"
        data-drop-path="releases/"
        data-active={drag.target?.path}
        onPointerDown={(event) =>
          drag.begin(event, "remote", ["releases/assets/"])
        }
      >
        <span
          data-entry-path="releases/assets/"
          data-entry-folder="releases/assets/"
        >
          remote folder
        </span>
      </section>
      {drag.dragging && (
        <div ref={drag.ghost}>Dragging {drag.dragging.paths.length}</div>
      )}
    </>
  );
}
function pointer(
  type: string,
  element: Element | Window,
  x: number,
  y: number,
) {
  fireEvent(
    element,
    new PointerEvent(type, {
      bubbles: true,
      pointerId: 7,
      pointerType: "mouse",
      button: 0,
      clientX: x,
      clientY: y,
    }),
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  native.listener = undefined;
  class TestPointerEvent extends MouseEvent {
    pointerId: number;
    pointerType: string;
    constructor(type: string, options: PointerEventInit) {
      super(type, options);
      this.pointerId = options.pointerId ?? 0;
      this.pointerType = options.pointerType ?? "mouse";
    }
  }
  vi.stubGlobal("PointerEvent", TestPointerEvent);
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: vi.fn((x: number) =>
      x >= 100
        ? screen.getByText("remote folder")
        : screen.getByText("local folder"),
    ),
  });
});
afterEach(() => {
  Reflect.deleteProperty(document, "elementFromPoint");
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});
describe("file drag and drop", () => {
  it("moves a selected group to a remote folder only after crossing the drag threshold", () => {
    render(<Harness />);
    pointer("pointerdown", screen.getByText("one.txt"), 10, 10);
    pointer("pointermove", window, 12, 10);
    expect(screen.queryByText("Dragging 2")).not.toBeInTheDocument();
    pointer("pointermove", window, 150, 10);
    expect(screen.getByText("Dragging 2")).toBeInTheDocument();
    expect(onSelect).toHaveBeenCalledWith("local", [
      "C:/one.txt",
      "C:/two.txt",
    ]);
    expect(screen.getByTestId("remote")).toHaveAttribute(
      "data-active",
      "releases/assets/",
    );
    pointer("pointerup", window, 150, 10);
    expect(onTransfer).toHaveBeenCalledWith(
      "local",
      ["C:/one.txt", "C:/two.txt"],
      { side: "remote", path: "releases/assets/" },
    );
    expect(screen.queryByText("Dragging 2")).not.toBeInTheDocument();
  });
  it("downloads folders into the hovered local folder and cancels a drag with Escape", () => {
    render(<Harness />);
    pointer("pointerdown", screen.getByText("remote folder"), 150, 10);
    pointer("pointermove", window, 10, 10);
    pointer("pointerup", window, 10, 10);
    expect(onTransfer).toHaveBeenCalledWith("remote", ["releases/assets/"], {
      side: "local",
      path: "C:/downloads/subfolder",
    });
    onTransfer.mockClear();
    pointer("pointerdown", screen.getByText("one.txt"), 10, 10);
    pointer("pointermove", window, 150, 10);
    fireEvent.keyDown(window, { key: "Escape" });
    pointer("pointerup", window, 150, 10);
    expect(onTransfer).not.toHaveBeenCalled();
  });
  it("rejects upload drops onto a read-only connection", () => {
    render(<Harness writable={false} />);
    pointer("pointerdown", screen.getByText("one.txt"), 10, 10);
    pointer("pointermove", window, 150, 10);
    pointer("pointerup", window, 150, 10);
    expect(onTransfer).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(
      expect.stringContaining("writable bucket"),
    );
  });
  it("maps native physical drop coordinates to the right folder and unregisters its listener", async () => {
    vi.stubGlobal("devicePixelRatio", 2);
    const view = render(<Harness />);
    await waitFor(() => expect(native.listener).toBeDefined());
    act(() =>
      native.listener?.({
        payload: {
          type: "drop",
          paths: ["C:/Explorer/file.txt"],
          position: { x: 300, y: 20 },
        } as DragDropEvent,
      }),
    );
    expect(document.elementFromPoint).toHaveBeenCalledWith(150, 10);
    expect(onExternalDrop).toHaveBeenCalledWith(["C:/Explorer/file.txt"], {
      side: "remote",
      path: "releases/assets/",
    });
    view.unmount();
    expect(native.stop).toHaveBeenCalledOnce();
  });
});
