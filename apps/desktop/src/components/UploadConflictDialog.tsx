import { useState } from "react";
import { ArrowRight, Files, Replace } from "lucide-react";
import { bytes, type ConflictAction, type Transfer } from "../lib/api";
import { Modal } from "./Modal";
import { Button } from "./ui/button";

function modified(seconds: number | null) {
  return seconds === null
    ? "Time unavailable"
    : new Date(seconds * 1000).toLocaleString();
}
export function UploadConflictDialog({
  job,
  onResolve,
  onClose,
  busy = false,
  error = "",
}: {
  job: Transfer;
  onResolve: (action: ConflictAction, all: boolean) => void;
  onClose: () => void;
  busy?: boolean;
  error?: string;
}) {
  const [applyAll, setApplyAll] = useState(false);
  const conflict = {
    key: job.key,
    path: job.local_path,
    source: { size: job.size, modified: job.source_modified ?? null },
    destination: job.conflict ?? { size: 0, modified: null },
    can_replace: job.conflict?.can_replace ?? false,
  };
  const canReplace = conflict.can_replace;
  function choose(action: ConflictAction) {
    if (!busy) onResolve(action, applyAll);
  }
  return (
    <Modal
      title="File already exists"
      description="This file is waiting for your decision. Other files continue uploading."
      wide
      onClose={onClose}
      closeDisabled={busy}
    >
      <div className="upload-conflict">
        <p className="conflict-key">{conflict.key}</p>
        <p className="muted">
          Choose what to do with the existing file in your bucket.
        </p>
        <div className="conflict-comparison">
          <section aria-label="Incoming file">
            <h3>
              <Files size={15} /> Incoming file
            </h3>
            <p className="conflict-path">{conflict.path}</p>
            <strong>{bytes(conflict.source.size)}</strong>
            <span>Modified {modified(conflict.source.modified)}</span>
          </section>
          <ArrowRight size={18} aria-hidden="true" />
          <section aria-label="Existing file">
            <h3>
              <Replace size={15} /> Existing file
            </h3>
            <p className="conflict-path">{conflict.key}</p>
            <strong>{bytes(conflict.destination.size)}</strong>
            <span>Modified {modified(conflict.destination.modified)}</span>
          </section>
        </div>
        <p className="muted">
          Replace updates the confirmed version. Keep both adds a numbered
          filename. Only this file waits; the rest of the batch continues.
        </p>
        {!canReplace && (
          <p role="status">
            The provider did not return an ETag. Choose Skip or Keep both.
          </p>
        )}
        {job.batch_id && (
          <label className="conflict-apply">
            <input
              type="checkbox"
              checked={applyAll}
              onChange={(event) => setApplyAll(event.target.checked)}
            />{" "}
            Apply to all conflicts in this upload, including files discovered
            later
          </label>
        )}
        {error && (
          <p className="error-box" role="alert">
            {error}
          </p>
        )}
        <div className="conflict-actions">
          <Button onClick={onClose} disabled={busy}>
            Decide later
          </Button>
          <div>
            <Button onClick={() => choose("skip")} disabled={busy} autoFocus>
              Skip
            </Button>
            <Button disabled={busy} onClick={() => choose("keep_both")}>
              Keep both
            </Button>
            <Button
              variant="destructive"
              disabled={busy || !canReplace}
              onClick={() => choose("replace")}
            >
              Replace
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
