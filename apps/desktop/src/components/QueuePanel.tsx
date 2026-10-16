import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  desktop,
  message,
  type QueuePage,
  type QueueSummary,
  type Transfer,
  type ConflictAction,
} from "../lib/api";
import { Queue } from "./Queue";
import { UploadConflictDialog } from "./UploadConflictDialog";

type Props = Omit<Parameters<typeof Queue>[0], "jobs" | "summary"> & {
  summary: QueueSummary;
};
export function QueuePanel({ summary, ...props }: Props) {
  const query = useQueryClient();
  const [filter, setFilter] = useState("all");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Transfer>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const page = useQuery({
    queryKey: ["transfers", "page", filter, offset],
    queryFn: () =>
      api<QueuePage>("transfer_page", { filter, offset, limit: 100 }),
    enabled: desktop && props.open,
    refetchInterval: props.open ? 1000 : false,
  });
  useEffect(() => {
    if (page.data && offset >= page.data.total && offset > 0)
      setOffset(Math.max(0, Math.floor((page.data.total - 1) / 100) * 100));
  }, [page.data, offset]);
  async function resolve(action: ConflictAction, all: boolean) {
    if (!selected || busy) return;
    setBusy(true);
    setError("");
    try {
      await api("resolve_upload", { id: selected.id, decision: action, all });
      setSelected(undefined);
      await query.invalidateQueries({ queryKey: ["transfers"] });
    } catch (error) {
      setError(message(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <Queue
        {...props}
        jobs={page.data?.items ?? []}
        summary={summary}
        filter={filter}
        onFilter={(filter) => {
          setFilter(filter);
          setOffset(0);
        }}
        pagination={{
          offset,
          total: page.data?.total ?? 0,
          previous: () => setOffset(Math.max(0, offset - 100)),
          next: () => setOffset(offset + 100),
        }}
        resolve={setSelected}
      />
      {page.error && props.open && (
        <p role="alert" className="error-box">
          {message(page.error)}
        </p>
      )}
      {selected && (
        <UploadConflictDialog
          key={selected.id}
          job={selected}
          onResolve={(action, all) => void resolve(action, all)}
          onClose={() => setSelected(undefined)}
          busy={busy}
          error={error}
        />
      )}
    </>
  );
}
