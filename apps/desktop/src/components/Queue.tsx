import { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronUp,
  Pause,
  Play,
  RotateCw,
  X,
} from "lucide-react";
import {
  bytes,
  type Transfer,
  type QueueSummary,
  type BatchOperation,
} from "../lib/api";
import { Button } from "./ui/button";
export function Queue({
  jobs,
  open,
  toggle,
  control,
  clear,
  bulkControl,
  bulkBusy = false,
  summary,
  filter,
  onFilter,
  pagination,
  resolve,
}: {
  jobs: Transfer[];
  summary?: QueueSummary;
  filter?: string;
  onFilter?: (filter: string) => void;
  pagination?: {
    offset: number;
    total: number;
    previous: () => void;
    next: () => void;
  };
  resolve?: (job: Transfer) => void;
  open: boolean;
  toggle: () => void;
  control: (id: string, operation: string) => void;
  clear: () => void;
  bulkControl?: (operation: BatchOperation) => void;
  bulkBusy?: boolean;
}) {
  const [localTab, setLocalTab] = useState("all");
  const tab = filter ?? localTab;
  const setTab = onFilter ?? setLocalTab;
  const [samples, setSamples] = useState<{ time: number; value: number }[]>([]);
  const total =
    summary?.bytes_done ?? jobs.reduce((sum, j) => sum + j.bytes_done, 0);
  useEffect(() => {
    const sample = () =>
      setSamples((old) => [
        ...old.slice(-59),
        { time: Date.now(), value: total },
      ]);
    sample();
    const interval = window.setInterval(sample, 1000);
    return () => window.clearInterval(interval);
  }, [total]);
  const last = samples.at(-1);
  const first = samples.at(-2);
  const rate =
    last && first
      ? Math.max(
          0,
          (last.value - first.value) /
            Math.max((last.time - first.time) / 1000, 0.01),
        )
      : 0;
  const active =
    (summary?.active ??
      jobs.filter((j) =>
        [
          "queued",
          "preparing",
          "uploading",
          "downloading",
          "verifying",
        ].includes(j.status),
      ).length) +
    (summary?.batches.filter((batch) => batch.state === "running").length ?? 0);
  const filtered = jobs.filter(
    (j) =>
      tab === "all" ||
      (tab === "active"
        ? [
            "queued",
            "preparing",
            "uploading",
            "downloading",
            "verifying",
          ].includes(j.status)
        : j.status === tab),
  );
  const rates = samples
    .slice(1)
    .map((s, i) =>
      Math.max(
        0,
        (s.value - samples[i].value) /
          Math.max((s.time - samples[i].time) / 1000, 0.01),
      ),
    );
  const max = Math.max(1, ...rates);
  const graph = rates
    .map(
      (r, i) =>
        `${(i / Math.max(1, rates.length - 1)) * 170},${28 - (r / max) * 25}`,
    )
    .join(" ");
  return (
    <section
      className={`transfer-panel ${open ? "expanded" : ""}`}
      aria-label="Transfer queue"
    >
      <div className="queue-heading">
        <button
          className="queue-title"
          onClick={toggle}
          aria-expanded={open}
          aria-controls="transfer-queue-content"
        >
          {open ? <ChevronDown size={15} /> : <ChevronUp size={15} />} Transfer
          queue <span className="count">{active}</span>
        </button>
        {!!summary?.counts.conflict && (
          <Button
            size="small"
            variant="ghost"
            className="queue-conflict-badge"
            onClick={() => {
              setTab("conflict");
              if (!open) toggle();
            }}
          >
            Review {summary.counts.conflict.toLocaleString()} conflicts
          </Button>
        )}
        {bulkControl && (
          <div
            className="queue-bulk-actions"
            aria-label="Control all transfers"
          >
            <Button
              variant="ghost"
              size="small"
              disabled={bulkBusy || !active}
              onClick={() => bulkControl("pause")}
            >
              <Pause size={13} /> Pause all
            </Button>
            <Button
              variant="ghost"
              size="small"
              disabled={
                bulkBusy ||
                !(summary
                  ? summary.counts.paused ||
                    summary.batches.some((batch) => batch.state === "paused")
                  : jobs.some((job) => job.status === "paused"))
              }
              onClick={() => bulkControl("resume")}
            >
              <Play size={13} /> Resume all
            </Button>
            <Button
              variant="ghost"
              size="small"
              className="cancel-all"
              disabled={
                bulkBusy ||
                !(summary
                  ? summary.cancellable ||
                    summary.batches.some((batch) =>
                      ["running", "paused"].includes(batch.state),
                    )
                  : jobs.some(
                      (job) =>
                        job.status !== "completed" &&
                        (job.status !== "cancelled" ||
                          !!job.upload_id ||
                          !!job.error),
                    ))
              }
              onClick={() => bulkControl("cancel")}
            >
              <X size={13} /> Cancel all
            </Button>
          </div>
        )}
        {open && (
          <select
            className="queue-filter"
            aria-label="Filter transfers"
            value={tab}
            onChange={(e) => setTab(e.target.value)}
          >
            {[
              "all",
              "active",
              "conflict",
              "completed",
              "failed",
              "paused",
              "skipped",
              "cancelled",
            ].map((value) => (
              <option key={value} value={value}>
                {value === "all"
                  ? `All (${summary?.total ?? jobs.length})`
                  : value.charAt(0).toUpperCase() + value.slice(1)}
              </option>
            ))}
          </select>
        )}
        <div className="queue-speed">
          <svg
            width="170"
            height="30"
            aria-label="Recent confirmed transfer throughput"
          >
            <polyline
              points={graph}
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
            />
          </svg>
          <span className="mono">{bytes(rate)}/s</span>
          <span className="muted">confirmed</span>
        </div>
        <Button
          variant="ghost"
          size="small"
          className="clear-finished"
          disabled={
            !(summary
              ? summary.clearable
              : jobs.some(
                  (j) =>
                    j.status === "completed" ||
                    (j.status === "cancelled" && !j.upload_id && !j.error),
                ))
          }
          onClick={clear}
        >
          Clear finished
        </Button>
      </div>
      {open && (
        <div id="transfer-queue-content" className="queue-content">
          {summary?.batches
            .filter(
              (batch) =>
                batch.state !== "finished" && batch.state !== "cancelled",
            )
            .map((batch) => (
              <div className="batch-progress" key={batch.id} role="status">
                <span>
                  {batch.state === "running"
                    ? "Discovering upload files…"
                    : `Upload discovery ${batch.state}`}{" "}
                  · {batch.discovered.toLocaleString()} queued
                </span>
                {batch.error && (
                  <span className="transfer-error">{batch.error}</span>
                )}
              </div>
            ))}
          {!!summary?.counts.conflict && (
            <button
              className="conflict-notice"
              onClick={() => setTab("conflict")}
            >
              {summary.counts.conflict.toLocaleString()} file(s) need a decision
              — review conflicts
            </button>
          )}
          <div className="queue-list">
            {!filtered.length ? (
              <div className="queue-empty">
                <div>
                  <Check size={18} />
                  <span>
                    {(summary?.total ?? jobs.length)
                      ? "No transfers in this view"
                      : "No transfers queued"}
                  </span>
                </div>
                <p>Queued uploads and downloads appear here.</p>
              </div>
            ) : (
              filtered.map((job) => (
                <div className="transfer-row" key={job.id}>
                  <span className={`direction ${job.direction}`}>
                    {job.direction === "upload" ? (
                      <ArrowUp size={15} />
                    ) : (
                      <ArrowDown size={15} />
                    )}
                  </span>
                  <div className="transfer-name">
                    <strong title={job.key}>{job.key}</strong>
                    <span className="muted" title={job.local_path}>
                      {job.local_path}
                    </span>
                    {job.error && (
                      <span className="transfer-error" role="alert">
                        {job.error}
                      </span>
                    )}
                  </div>
                  <div className="transfer-progress">
                    <div>
                      <span className={`transfer-status ${job.status}`}>
                        {job.status}
                      </span>
                      <span className="mono">
                        {job.size
                          ? Math.min(
                              100,
                              (job.bytes_done / job.size) * 100,
                            ).toFixed(0)
                          : job.status === "completed"
                            ? "100"
                            : "0"}
                        %
                      </span>
                    </div>
                    <progress
                      max={Math.max(1, job.size)}
                      value={
                        job.size
                          ? job.bytes_done
                          : job.status === "completed"
                            ? 1
                            : 0
                      }
                    />
                    <span className="mono muted">
                      {bytes(job.bytes_done)} / {bytes(job.size)}
                    </span>
                  </div>
                  <div className="transfer-actions">
                    {job.status === "conflict" && resolve && (
                      <Button size="small" onClick={() => resolve(job)}>
                        Resolve
                      </Button>
                    )}
                    {[
                      "queued",
                      "preparing",
                      "uploading",
                      "downloading",
                    ].includes(job.status) && (
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Pause ${job.key}`}
                        onClick={() => control(job.id, "pause")}
                      >
                        <Pause size={14} />
                      </Button>
                    )}
                    {["paused", "failed"].includes(job.status) && (
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Resume ${job.key}`}
                        onClick={() => control(job.id, "resume")}
                      >
                        {job.status === "failed" ? (
                          <RotateCw size={14} />
                        ) : (
                          <Play size={14} />
                        )}
                      </Button>
                    )}
                    {(!["completed", "cancelled"].includes(job.status) ||
                      (job.status === "cancelled" &&
                        (!!job.upload_id || !!job.error))) && (
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Cancel ${job.key}`}
                        onClick={() => control(job.id, "cancel")}
                      >
                        <X size={14} />
                      </Button>
                    )}
                  </div>
                </div>
              ))
            )}
          </div>
          {pagination && pagination.total > 0 && (
            <div className="queue-pagination">
              <span>
                {pagination.offset + 1}–
                {Math.min(pagination.offset + 100, pagination.total)} of{" "}
                {pagination.total.toLocaleString()} in this view
              </span>
              <Button
                size="small"
                disabled={!pagination.offset}
                onClick={pagination.previous}
              >
                Previous page
              </Button>
              <Button
                size="small"
                disabled={pagination.offset + 100 >= pagination.total}
                onClick={pagination.next}
              >
                Next page
              </Button>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
