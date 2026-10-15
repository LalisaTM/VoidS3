import { useCallback, useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { desktop, message } from "./api";

export interface UpdateInfo {
  version: string;
  notes: string;
}
export type UpdateProgress =
  | { stage: "downloading"; downloaded: number; total: number | null }
  | { stage: "verifying" | "installing" };
export interface UpdateState {
  status:
    | "idle"
    | "checking"
    | "current"
    | "available"
    | "unsupported"
    | "error"
    | UpdateProgress["stage"];
  update?: UpdateInfo;
  error?: string;
  downloaded: number;
  total: number | null;
}
export function useUpdater(automatic: boolean) {
  const [state, setState] = useState<UpdateState>({
    status: desktop ? "idle" : "unsupported",
    downloaded: 0,
    total: null,
  });
  const operation = useRef(false);
  const check = useCallback(async () => {
    if (!desktop || operation.current) return;
    operation.current = true;
    setState((old) => ({ ...old, status: "checking", error: undefined }));
    try {
      const result = await invoke<{
        supported: boolean;
        update: UpdateInfo | null;
      }>("check_for_updates");
      setState({
        status: !result.supported
          ? "unsupported"
          : result.update
            ? "available"
            : "current",
        update: result.update ?? undefined,
        downloaded: 0,
        total: null,
      });
    } catch (error) {
      setState((old) => ({ ...old, status: "error", error: message(error) }));
    } finally {
      operation.current = false;
    }
  }, []);
  useEffect(() => {
    if (!desktop || !automatic) return;
    const start = window.setTimeout(() => void check(), 3000);
    const daily = window.setInterval(() => void check(), 24 * 60 * 60 * 1000);
    return () => {
      window.clearTimeout(start);
      window.clearInterval(daily);
    };
  }, [automatic, check]);
  async function install() {
    if (!desktop || !state.update || operation.current) return;
    operation.current = true;
    setState((old) => ({
      ...old,
      status: "downloading",
      error: undefined,
      downloaded: 0,
      total: null,
    }));
    const onEvent = new Channel<UpdateProgress>();
    onEvent.onmessage = (event) =>
      setState((old) => ({
        ...old,
        status: event.stage,
        ...(event.stage === "downloading"
          ? { downloaded: event.downloaded, total: event.total }
          : {}),
      }));
    try {
      await invoke("install_update", {
        version: state.update.version,
        onEvent,
      });
      setState((old) => ({ ...old, status: "installing" }));
    } catch (error) {
      setState((old) => ({ ...old, status: "error", error: message(error) }));
    } finally {
      operation.current = false;
    }
  }
  const busy = ["downloading", "verifying", "installing"].includes(
    state.status,
  );
  return { ...state, busy, check, install };
}
export type Updater = ReturnType<typeof useUpdater>;
