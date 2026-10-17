import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FileEntry } from "../lib/api";
import {
  orderFiles,
  type FileOrder,
  type OrderedFiles,
} from "../lib/fileOrder";
import { useFileOrder } from "../lib/useFileOrder";

const order: FileOrder = { search: "", sort: "name", descending: false };
const files: FileEntry[] = Array.from({ length: 10_000 }, (_, index) => ({
  name: `${String(10_000 - index).padStart(5, "0")}.JPG`,
  path: `photos/${index}`,
  size: index,
  modified: index,
  is_dir: false,
  etag: null,
}));
class BackgroundWorker {
  static instances: BackgroundWorker[] = [];
  onmessage: ((event: { data: OrderedFiles & { id: number } }) => void) | null =
    null;
  onerror: (() => void) | null = null;
  postMessage = vi.fn();
  terminate = vi.fn();
  constructor() {
    BackgroundWorker.instances.push(this);
  }
  finish(id: number, result: OrderedFiles) {
    act(() => this.onmessage?.({ data: { id, ...result } }));
  }
}
afterEach(() => {
  vi.unstubAllGlobals();
  BackgroundWorker.instances = [];
});

describe("background directory ordering", () => {
  it("sorts 10,000 files, keeps folders first, filters case-insensitively and counts only file bytes", () => {
    const folder = {
      ...files[0],
      name: "Assets.JPG",
      is_dir: true,
      size: 999_999,
    };
    const input = [...files, folder];
    const result = orderFiles(input, {
      search: ".jpg",
      sort: "size",
      descending: true,
    });
    expect(result.indices).toHaveLength(10_001);
    expect(input[result.indices[0]]).toBe(folder);
    expect(input[result.indices[1]].size).toBe(9999);
    expect(result.bytes).toBe((9999 * 10_000) / 2);
    expect(input[0]).toBe(files[0]);
    expect(
      orderFiles(input, { ...order, search: "no matching file" }).indices,
    ).toHaveLength(0);
  });
  it("uses a worker for large listings, reuses its dataset and rejects stale results", () => {
    vi.stubGlobal("Worker", BackgroundWorker);
    const { result, rerender, unmount } = renderHook(
      ({ options }) => useFileOrder(files, options),
      { initialProps: { options: order } },
    );
    const worker = BackgroundWorker.instances[0];
    expect(result.current.ordering).toBe(true);
    expect(result.current.filtered).toHaveLength(0);
    const first = worker.postMessage.mock.calls[0][0];
    expect(first.entries).toBe(files);
    worker.finish(first.id, orderFiles(files, order));
    expect(result.current.filtered[0].name).toBe("00001.JPG");
    expect(result.current.ordering).toBe(false);
    const next = { ...order, search: "00001" };
    rerender({ options: next });
    const second = worker.postMessage.mock.calls[1][0];
    expect(second).not.toHaveProperty("entries");
    worker.finish(first.id, orderFiles(files, order));
    expect(result.current.ordering).toBe(true);
    worker.finish(second.id, orderFiles(files, next));
    expect(result.current.filtered).toHaveLength(1);
    expect(result.current.totalBytes).toBe(9999);
    unmount();
    expect(worker.terminate).toHaveBeenCalledOnce();
  });
  it("terminates workers for obsolete listings and exposes failure without sorting a large listing on the UI thread", () => {
    vi.stubGlobal("Worker", BackgroundWorker);
    const { result, rerender } = renderHook(
      ({ entries }) => useFileOrder(entries, order),
      { initialProps: { entries: files } },
    );
    const first = BackgroundWorker.instances[0];
    rerender({ entries: files.slice(0, 1000) });
    expect(first.terminate).toHaveBeenCalledOnce();
    const second = BackgroundWorker.instances[1];
    act(() => second.onerror?.());
    expect(result.current.error).toMatch(/Background file sorting failed/);
    expect(result.current.ordering).toBe(false);
    expect(result.current.filtered).toHaveLength(0);
  });
  it("keeps small listings bounded and synchronous without starting a worker", () => {
    vi.stubGlobal("Worker", BackgroundWorker);
    const { result } = renderHook(() =>
      useFileOrder(files.slice(0, 20), order),
    );
    expect(result.current.filtered).toHaveLength(20);
    expect(result.current.ordering).toBe(false);
    expect(BackgroundWorker.instances).toHaveLength(0);
  });
});
