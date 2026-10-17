import { orderFiles, type FileOrder } from "./fileOrder";
import type { FileEntry } from "./api";

let entries: FileEntry[] = [];
let pending: { id: number; order: FileOrder } | undefined;
let scheduled = false;
self.onmessage = (
  event: MessageEvent<{ entries?: FileEntry[]; id: number; order: FileOrder }>,
) => {
  if (event.data.entries) entries = event.data.entries;
  pending = event.data;
  if (scheduled) return;
  scheduled = true;
  // Coalesce rapidly typed filter requests before starting the next sort.
  setTimeout(() => {
    scheduled = false;
    const request = pending!;
    const result = orderFiles(entries, request.order);
    self.postMessage(
      { id: request.id, ...result },
      { transfer: [result.indices.buffer] },
    );
  }, 0);
};
