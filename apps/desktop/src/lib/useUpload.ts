import { useRef, useState } from "react";
import { api } from "./api";

// Starting a batch only registers background discovery. The queue owns progress,
// cancellation and conflict decisions, so large batches never hold a modal open.
export function useUpload({ onStarted }: { onStarted: (id: string) => void }) {
  const locked = useRef(false);
  const [starting, setStarting] = useState(false);
  async function start(id: string, paths: string[], prefix: string) {
    if (locked.current) return;
    locked.current = true;
    setStarting(true);
    try {
      const batch = await api<string>("start_upload", { id, paths, prefix });
      onStarted(batch);
    } finally {
      locked.current = false;
      setStarting(false);
    }
  }
  return { start, starting };
}
