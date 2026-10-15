import { useState } from "react";
import { ArrowDown, ArrowUp, CircleAlert, RefreshCw } from "lucide-react";
import { api, bytes, message, type Change, type Profile } from "../lib/api";
import { Modal } from "./Modal";
import { Button } from "./ui/button";
export function SyncDialog({
  profile,
  local,
  prefix,
  onClose,
  onApplied,
}: {
  profile: Profile;
  local: string;
  prefix: string;
  onClose: () => void;
  onApplied: () => void;
}) {
  const [mode, setMode] = useState("upload");
  const [changes, setChanges] = useState<Change[]>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  async function run(apply: boolean) {
    setBusy(true);
    setError("");
    try {
      const result = await api<Change[]>(
        apply ? "sync_apply" : "sync_preview",
        { id: profile.id, local, prefix, mode },
      );
      if (apply) {
        onApplied();
        onClose();
      } else setChanges(result);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  const additions =
    changes?.filter((c) => c.action === "upload" || c.action === "download")
      .length ?? 0;
  return (
    <Modal
      title="Synchronize folders"
      description="Preview real differences before queuing transfers. No files are deleted or overwritten."
      onClose={onClose}
      wide
    >
      <div className="form">
        <div className="sync-path">
          <span>LOCAL</span>
          <code>{local}</code>
          <span>REMOTE</span>
          <code>
            {profile.bucket}/{prefix}
          </code>
        </div>
        <div className="toolbar">
          <label className="grow">
            Direction
            <select
              value={mode}
              onChange={(e) => {
                setMode(e.target.value);
                setChanges(undefined);
              }}
            >
              <option value="upload">Local → remote</option>
              <option value="download">Remote → local</option>
              <option value="bidirectional">Bidirectional additions</option>
            </select>
          </label>
          <Button onClick={() => void run(false)} disabled={busy}>
            <RefreshCw size={14} /> {busy ? "Scanning…" : "Compare folders"}
          </Button>
        </div>
        <p className="notice">
          <CircleAlert size={15} /> Existing pairs remain conflicts, including
          equal-size files. This release copies missing files only; mirror
          deletion and overwrite policies are unfinished.
        </p>
        {error && (
          <p className="error-box" role="alert">
            {error}
          </p>
        )}
        {changes && (
          <>
            <div className="section-label">
              {additions} ADDITIONS ·{" "}
              {changes.filter((c) => c.action === "conflict").length} CONFLICTS
            </div>
            <div className="sync-results">
              {changes.length === 0 ? (
                <p className="muted">No files in either folder.</p>
              ) : (
                changes.map((c) => (
                  <div key={c.key} title={c.reason}>
                    {c.action === "upload" ? (
                      <ArrowUp size={14} />
                    ) : c.action === "download" ? (
                      <ArrowDown size={14} />
                    ) : (
                      <CircleAlert size={14} />
                    )}
                    <code>{c.key}</code>
                    <span className={`tag ${c.action}`}>{c.action}</span>
                    <span className="mono muted">{bytes(c.size)}</span>
                  </div>
                ))
              )}
            </div>
          </>
        )}
        <div className="modal-footer">
          <span className="muted">
            Re-scanned at apply time. Conflicts are preserved.
          </span>
          <Button
            variant="default"
            disabled={
              !additions || busy || (profile.read_only && mode !== "download")
            }
            onClick={() => void run(true)}
          >
            Queue {additions} additions
          </Button>
        </div>
      </div>
    </Modal>
  );
}
