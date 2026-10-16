import { useEffect, useRef, useState } from "react";
import {
  api,
  message,
  type PreparedUpload,
  type UploadChoice,
  type UploadResult,
} from "./api";

export function useUpload({
  onComplete,
  onError,
}: {
  onComplete: (result: UploadResult) => void;
  onError: (error: string) => void;
}) {
  const [plan, setPlan] = useState<PreparedUpload>();
  const [phase, setPhase] = useState<"checking" | "confirming" | "queueing">();
  const locked = useRef(false);
  const pending = useRef<string | undefined>(undefined);
  const cancelled = useRef(false);
  useEffect(
    () => () => {
      cancelled.current = true;
      if (pending.current)
        void api("discard_upload", { id: pending.current }).catch(() => {});
    },
    [],
  );
  function reset() {
    locked.current = false;
    pending.current = undefined;
    setPlan(undefined);
    setPhase(undefined);
  }
  async function commit(id: string, choices: UploadChoice[]) {
    setPhase("queueing");
    pending.current = undefined; // Commit consumes the backend plan exactly once.
    try {
      onComplete(await api<UploadResult>("commit_upload", { id, choices }));
    } catch (error) {
      onError(`${message(error)} Upload again to recheck destinations.`);
    } finally {
      reset();
    }
  }
  async function start(id: string, paths: string[], prefix: string) {
    if (locked.current)
      throw new Error(
        "Finish or cancel the current upload confirmation first.",
      );
    locked.current = true;
    cancelled.current = false;
    setPhase("checking");
    try {
      const prepared = await api<PreparedUpload>("prepare_upload", {
        id,
        paths,
        prefix,
      });
      if (cancelled.current) {
        await api("discard_upload", { id: prepared.id });
        reset();
      } else if (prepared.conflicts.length) {
        pending.current = prepared.id;
        setPlan(prepared);
        setPhase("confirming");
      } else {
        await commit(prepared.id, []);
      }
    } catch (error) {
      const ignore = cancelled.current;
      reset();
      if (!ignore) throw error;
    }
  }
  async function cancel() {
    if (phase === "queueing") return;
    cancelled.current = true;
    // A running preflight must settle before another one can start.
    if (phase === "checking") {
      setPhase(undefined);
      return;
    }
    const id = pending.current;
    reset();
    if (id) {
      try {
        await api("discard_upload", { id });
      } catch (error) {
        onError(message(error));
      }
    }
  }
  return {
    plan,
    phase,
    start,
    cancel,
    resolve: (choices: UploadChoice[]) => {
      if (pending.current) void commit(pending.current, choices);
    },
  };
}
