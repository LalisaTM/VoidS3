import { useEffect, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronUp,
  ListFilter,
  Pause,
  Play,
  RotateCw,
  X,
} from "lucide-react";
import { bytes, type Transfer } from "../lib/api";
import { Button } from "./ui/button";
export function Queue({
  jobs,
  open,
  toggle,
  control,
  clear,
}: {
  jobs: Transfer[];
  open: boolean;
  toggle: () => void;
  control: (id: string, operation: string) => void;
  clear: () => void;
}) {
  const [tab, setTab] = useState("all");
  const [samples, setSamples] = useState<{ time: number; value: number }[]>([]);
  const total = jobs.reduce((sum, j) => sum + j.bytes_done, 0);
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
  const active = jobs.filter((j) =>
    ["queued", "preparing", "uploading", "downloading", "verifying"].includes(
      j.status,
    ),
  ).length;
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
        <button className="queue-title" onClick={toggle}>
          {open ? <ChevronDown size={15} /> : <ChevronUp size={15} />} Transfer
          queue <span className="count">{active}</span>
        </button>
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
          disabled={!jobs.some((j) => j.status === "completed")}
          onClick={clear}
        >
          Clear finished
        </Button>
      </div>
      {open && (
        <>
          <div className="queue-tabs">
            {["all", "active", "completed", "failed", "paused"].map((value) => (
              <button
                key={value}
                className={tab === value ? "active" : ""}
                onClick={() => setTab(value)}
              >
                {value.charAt(0).toUpperCase() + value.slice(1)}
                {value === "all" && <span>{jobs.length}</span>}
              </button>
            ))}
            <ListFilter size={14} className="muted" />
          </div>
          <div className="queue-list">
            {!filtered.length ? (
              <div className="queue-empty">
                <div>
                  <Check size={18} />
                  <span>
                    {jobs.length
                      ? "No transfers in this view"
                      : "All clear. Ready when you are."}
                  </span>
                </div>
                <p>
                  Transfers run in Rust. Upload progress counts
                  provider-confirmed parts.
                </p>
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
                      (job.status === "cancelled" && !!job.upload_id)) && (
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
        </>
      )}
    </section>
  );
}
