import { useEffect, useMemo, useRef, useState } from "react";
import type { FileEntry } from "./api";
import { orderFiles, type FileOrder, type OrderedFiles } from "./fileOrder";

const INLINE_LIMIT = 500;
const empty: FileEntry[] = [];

export function useFileOrder(entries: FileEntry[], order: FileOrder) {
  const { search, sort, descending } = order;
  const large = entries.length > INLINE_LIMIT;
  const worker = useRef<Worker | null>(null);
  const loaded = useRef<Worker | null>(null);
  const sequence = useRef(0);
  const [response, setResponse] = useState<{
    source: FileEntry[];
    search: string;
    sort: FileOrder["sort"];
    descending: boolean;
    result?: OrderedFiles;
    error?: string;
  }>();
  const inline = useMemo(
    () =>
      large ? undefined : orderFiles(entries, { search, sort, descending }),
    [entries, large, search, sort, descending],
  );

  useEffect(() => {
    if (!large) return;
    try {
      worker.current = new Worker(
        new URL("./fileOrder.worker.ts", import.meta.url),
        { type: "module" },
      );
    } catch {
      worker.current = null;
    }
    return () => {
      worker.current?.terminate();
      worker.current = null;
    };
  }, [entries, large]);

  useEffect(() => {
    if (!large) return;
    const current = worker.current;
    const id = ++sequence.current;
    const snapshot = { source: entries, search, sort, descending };
    if (!current) {
      setResponse({
        ...snapshot,
        error:
          "Background file sorting is unavailable. Reload the app to retry.",
      });
      return;
    }
    current.onmessage = (
      event: MessageEvent<OrderedFiles & { id: number }>,
    ) => {
      if (event.data.id === sequence.current)
        setResponse({ ...snapshot, result: event.data });
    };
    current.onerror = () => {
      current.terminate();
      if (worker.current === current) worker.current = null;
      setResponse({
        ...snapshot,
        error: "Background file sorting failed. Reload the app to retry.",
      });
    };
    // The worker retains the listing. Filter/sort changes only send small options.
    current.postMessage({
      id,
      order: { search, sort, descending },
      ...(loaded.current === current ? {} : { entries }),
    });
    loaded.current = current;
    return () => {
      current.onmessage = null;
      current.onerror = null;
    };
  }, [entries, large, search, sort, descending]);

  const current =
    response?.source === entries &&
    response.search === search &&
    response.sort === sort &&
    response.descending === descending
      ? response
      : undefined;
  const result = inline ?? current?.result;
  const filtered = useMemo(
    () =>
      result ? Array.from(result.indices, (index) => entries[index]) : empty,
    [entries, result],
  );
  return {
    filtered,
    totalBytes: result?.bytes ?? 0,
    ordering: large && !current,
    error: large ? current?.error : undefined,
  };
}
