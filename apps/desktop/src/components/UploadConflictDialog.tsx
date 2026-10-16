import { useState } from "react";
import { ArrowRight, Files, Replace } from "lucide-react";
import {
  bytes,
  type ConflictAction,
  type PreparedUpload,
  type UploadChoice,
} from "../lib/api";
import { Modal } from "./Modal";
import { Button } from "./ui/button";

function modified(seconds: number | null) {
  return seconds === null
    ? "Time unavailable"
    : new Date(seconds * 1000).toLocaleString();
}
export function UploadConflictDialog({
  plan,
  onResolve,
  onCancel,
}: {
  plan: PreparedUpload;
  onResolve: (choices: UploadChoice[]) => void;
  onCancel: () => void;
}) {
  const [choices, setChoices] = useState<UploadChoice[]>([]);
  const [applyAll, setApplyAll] = useState(false);
  const conflict = plan.conflicts[choices.length];
  const remaining = plan.conflicts.length - choices.length;
  const canReplace = applyAll
    ? plan.conflicts.slice(choices.length).every((item) => item.can_replace)
    : conflict.can_replace;
  function choose(action: ConflictAction) {
    const next = [
      ...choices,
      ...(applyAll ? plan.conflicts.slice(choices.length) : [conflict]).map(
        ({ index }) => ({ index, action }),
      ),
    ];
    if (next.length === plan.conflicts.length) onResolve(next);
    else setChoices(next);
  }
  return (
    <Modal
      title="File already exists"
      description={`Conflict ${choices.length + 1} of ${plan.conflicts.length} · ${plan.total} file(s) in this upload`}
      wide
      onClose={onCancel}
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
          filename. Nothing in this batch starts until every conflict is
          resolved.
        </p>
        {!canReplace && (
          <p role="status">
            The provider did not return an ETag. Choose Skip or Keep both.
          </p>
        )}
        {remaining > 1 && (
          <label className="conflict-apply">
            <input
              type="checkbox"
              checked={applyAll}
              onChange={(event) => setApplyAll(event.target.checked)}
            />{" "}
            Apply to all remaining conflicts in this upload ({remaining})
          </label>
        )}
        <div className="conflict-actions">
          <Button onClick={onCancel}>Cancel upload</Button>
          <div>
            <Button onClick={() => choose("skip")} autoFocus>
              Skip
            </Button>
            <Button onClick={() => choose("keep_both")}>Keep both</Button>
            <Button
              variant="destructive"
              disabled={!canReplace}
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
