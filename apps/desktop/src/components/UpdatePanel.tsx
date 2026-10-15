import { useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { bytes, desktop } from "../lib/api";
import type { Updater } from "../lib/useUpdater";
import { version } from "../../package.json";
import { Button } from "./ui/button";

export function UpdatePanel({
  updater,
  activeTransfers,
}: {
  updater: Updater;
  activeTransfers: number;
}) {
  const [confirm, setConfirm] = useState(false);
  const percent =
    updater.total && updater.total > 0
      ? Math.min(100, Math.round((updater.downloaded / updater.total) * 100))
      : undefined;
  return (
    <section className="update-panel" aria-label="Application updates">
      <div className="update-heading">
        <div>
          <h3>Application updates</h3>
          <p>VoidS3 {version} · GitHub Releases</p>
        </div>
        <Button
          size="small"
          disabled={
            !desktop ||
            updater.busy ||
            updater.status === "checking" ||
            updater.status === "unsupported"
          }
          onClick={() => void updater.check()}
        >
          <RefreshCw size={13} />
          {updater.status === "checking" ? "Checking…" : "Check for updates"}
        </Button>
      </div>
      {updater.status === "unsupported" ? (
        <p className="muted">
          {desktop
            ? "Self-updating is available for Windows x64 installers. Download other packages from GitHub Releases."
            : "Update checks are available in the Windows desktop app."}
        </p>
      ) : (
        <>
          {updater.status === "current" && (
            <p role="status">You are up to date.</p>
          )}
          {updater.status === "idle" && (
            <p className="muted">
              Checks the latest published release. Updates install only when you
              choose.
            </p>
          )}
          {updater.update && (
            <>
              <p>
                <strong>Version {updater.update.version} is available</strong>
              </p>
              {updater.update.notes && (
                <details className="update-notes">
                  <summary>Release notes</summary>
                  <pre>{updater.update.notes}</pre>
                </details>
              )}
              {activeTransfers > 0 && (
                <p className="update-warning">
                  Pause or cancel all transfers and wait for them to stop before
                  installing.
                </p>
              )}
              {!updater.busy && !confirm && (
                <Button
                  size="small"
                  variant="default"
                  disabled={
                    activeTransfers > 0 || updater.status === "checking"
                  }
                  onClick={() => setConfirm(true)}
                >
                  <Download size={14} /> Install update…
                </Button>
              )}
              {!updater.busy && confirm && (
                <div className="update-confirm">
                  <p>
                    Install version {updater.update.version}? VoidS3 will close,
                    update, and reopen. Paused transfers and saved connections
                    remain.
                  </p>
                  <div>
                    <Button size="small" onClick={() => setConfirm(false)}>
                      Not now
                    </Button>
                    <Button
                      size="small"
                      variant="default"
                      disabled={
                        activeTransfers > 0 || updater.status === "checking"
                      }
                      onClick={() => {
                        setConfirm(false);
                        void updater.install();
                      }}
                    >
                      Install and restart
                    </Button>
                  </div>
                </div>
              )}
            </>
          )}
          {updater.busy && (
            <div className="update-progress" role="status" aria-live="polite">
              <span>
                {updater.status === "downloading"
                  ? `Downloading update: ${bytes(updater.downloaded)}${updater.total ? ` / ${bytes(updater.total)}` : ""}`
                  : updater.status === "verifying"
                    ? "Verifying update signature…"
                    : "Installing update. VoidS3 will restart…"}
              </span>
              <progress
                aria-label="Update download progress"
                max={100}
                value={updater.status === "downloading" ? percent : undefined}
              />
            </div>
          )}
          {updater.error && (
            <p role="alert" className="error-box">
              {updater.error}
            </p>
          )}
        </>
      )}
    </section>
  );
}
