import { useState, type FormEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, message, type TransferSettings } from "../lib/api";
import { usePreferences } from "../store";
import { version } from "../../package.json";
import { Modal } from "./Modal";
import { Button } from "./ui/button";
import { UpdatePanel } from "./UpdatePanel";
import type { Updater } from "../lib/useUpdater";
export function SettingsDialog({
  onClose,
  updater,
  activeTransfers,
}: {
  onClose: () => void;
  updater: Updater;
  activeTransfers: number;
}) {
  const { data, error } = useQuery({
    queryKey: ["settings"],
    queryFn: () => api<TransferSettings>("settings"),
  });
  const [draft, setDraft] = useState<TransferSettings>();
  const [failure, setFailure] = useState("");
  const [busy, setBusy] = useState(false);
  const settings = draft ?? data;
  const preferences = usePreferences();
  const queryClient = useQueryClient();
  async function save(e: FormEvent) {
    e.preventDefault();
    if (!settings || updater.busy) return;
    setBusy(true);
    try {
      await api("save_settings", { settings });
      await queryClient.invalidateQueries({ queryKey: ["settings"] });
      onClose();
    } catch (e) {
      setFailure(message(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      title="Settings"
      description="Conservative defaults. Tune only after measuring real transfers."
      onClose={onClose}
      closeDisabled={updater.busy}
    >
      <form onSubmit={(e) => void save(e)} className="form">
        <UpdatePanel updater={updater} activeTransfers={activeTransfers} />
        <label className="checkbox">
          <input
            type="checkbox"
            checked={preferences.checkUpdatesAutomatically}
            onChange={(e) =>
              preferences.set({ checkUpdatesAutomatically: e.target.checked })
            }
          />{" "}
          Check for updates at startup and once a day
        </label>
        <div className="section-label">APPEARANCE</div>
        <div className="form-grid">
          <label>
            Theme
            <select
              value={preferences.theme}
              onChange={(e) =>
                preferences.set({ theme: e.target.value as "dark" | "light" })
              }
            >
              <option value="dark">Dark / charcoal</option>
              <option value="light">Light / paper</option>
            </select>
          </label>
          <label>
            Pane layout
            <select
              value={preferences.view}
              onChange={(e) =>
                preferences.set({
                  view: e.target.value as "dual" | "local" | "remote",
                })
              }
            >
              <option value="dual">Dual pane</option>
              <option value="remote">Remote only</option>
              <option value="local">Local only</option>
            </select>
          </label>
        </div>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={preferences.hidden}
            onChange={(e) => preferences.set({ hidden: e.target.checked })}
          />{" "}
          Show local dotfiles
        </label>
        <div className="section-label">TRANSFER ENGINE</div>
        {error && <p className="error-box">{message(error)}</p>}
        {settings && (
          <div className="form-grid">
            {(
              [
                ["concurrent_files", "Concurrent files", 1, 16, 1],
                ["concurrent_parts", "Parts per file", 1, 16, 1],
                ["max_requests", "Global request limit", 1, 64, 1],
                ["part_size", "Part size (MiB)", 5, 512, 1048576],
                [
                  "multipart_threshold",
                  "Multipart threshold (MiB)",
                  5,
                  5120,
                  1048576,
                ],
                ["retries", "SDK retries per request", 0, 10, 1],
              ] as const
            ).map(([key, label, min, max, divisor]) => (
              <label key={key}>
                {label}
                <input
                  type="number"
                  required
                  min={min}
                  max={max}
                  value={settings[key] / divisor}
                  onChange={(e) =>
                    setDraft({
                      ...settings,
                      [key]: Number(e.target.value) * divisor,
                    })
                  }
                />
              </label>
            ))}
          </div>
        )}
        <p className="muted">
          File and global request limits take effect after restarting VoidS3.
          Part settings apply to newly queued transfers. File bodies stream from
          disk; part sizes do not allocate whole-part buffers.
        </p>
        {failure && (
          <p role="alert" className="error-box">
            {failure}
          </p>
        )}
        <div className="modal-footer">
          <span className="muted mono">VoidS3 {version} · MIT</span>
          <Button
            type="submit"
            variant="default"
            disabled={!settings || busy || updater.busy}
          >
            Save settings
          </Button>
        </div>
      </form>
    </Modal>
  );
}
