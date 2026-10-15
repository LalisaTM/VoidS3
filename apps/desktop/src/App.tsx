import { useEffect, useState, type ReactNode } from "react";
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useFileDragDrop } from "./lib/useFileDragDrop";
import {
  ArrowDownToLine,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ArrowUpFromLine,
  Archive,
  Bookmark,
  Check,
  ChevronDown,
  Cloud,
  CloudOff,
  Copy,
  FolderOpen,
  FolderPlus,
  HardDrive,
  LockKeyhole,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Trash2,
  Terminal,
  Unplug,
  Workflow,
  X,
} from "lucide-react";
import * as Dropdown from "@radix-ui/react-dropdown-menu";
import {
  api,
  bytes,
  desktop,
  message,
  parent,
  type FileEntry,
  type BatchOperation,
  type BatchControlResult,
  type DeleteResult,
  type Page,
  type Profile,
  type Transfer,
} from "./lib/api";
import { usePreferences } from "./store";
import { version } from "../package.json";
import { Button } from "./components/ui/button";
import { ConnectionDialog } from "./components/ConnectionDialog";
import { WorkspaceHeader } from "./components/WorkspaceHeader";
import { SplitHandle } from "./components/SplitHandle";
import { Files } from "./components/Files";
import { Modal } from "./components/Modal";
import { Queue } from "./components/Queue";
import { SettingsDialog } from "./components/SettingsDialog";
import { SyncDialog } from "./components/SyncDialog";

function useNavigation(initial: string) {
  const [history, setHistory] = useState<string[]>([]);
  const [position, setPosition] = useState(-1);
  const path = position >= 0 ? history[position] : initial;
  function go(next: string) {
    setHistory((old) => [
      ...(position < 0 ? [initial] : old.slice(0, position + 1)),
      next,
    ]);
    setPosition((old) => (old < 0 ? 1 : old + 1));
  }
  return {
    path,
    go,
    back: () => setPosition((p) => Math.max(0, p - 1)),
    forward: () => setPosition((p) => Math.min(history.length - 1, p + 1)),
    canBack: position > 0,
    canForward: position >= 0 && position < history.length - 1,
  };
}
interface Prompt {
  title: string;
  description: string;
  input?: string;
  label?: string;
  destructive?: boolean;
  content?: ReactNode;
  keepOpen?: boolean;
  execute: (value: string) => Promise<void>;
}
export default function App() {
  const query = useQueryClient();
  const preferences = usePreferences();
  const [connectionId, setConnectionId] = useState("");
  const [connectionForm, setConnectionForm] = useState<Profile | "new">();
  const [settings, setSettings] = useState(false);
  const [mainView, setMainView] = useState<"files" | "transfers">("files");
  const [bookmarksOpen, setBookmarksOpen] = useState(false);
  const [activityOpen, setActivityOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [focusedSide, setFocusedSide] = useState<"local" | "remote">("local");
  const [navigationOpen, setNavigationOpen] = useState(false);
  useEffect(() => {
    if (navigationOpen)
      document
        .querySelector<HTMLElement>("#workspace-navigation button")
        ?.focus();
  }, [navigationOpen]);
  const [sync, setSync] = useState(false);
  const [bulkBusy, setBulkBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [prompt, setPrompt] = useState<Prompt>();
  const [localSelection, setLocalSelection] = useState<string[]>([]);
  const [remoteSelection, setRemoteSelection] = useState<string[]>([]);
  const [remoteStart, setRemoteStart] = useState("");
  const profiles = useQuery({
    queryKey: ["profiles"],
    queryFn: () => api<Profile[]>("profiles"),
    enabled: desktop,
  });
  const profile = profiles.data?.find((p) => p.id === connectionId);
  const home = useQuery({
    queryKey: ["home"],
    queryFn: () => api<string>("home"),
    enabled: desktop,
  });
  const local = useNavigation(home.data ?? "");
  const remote = useNavigation(remoteStart);
  const localFiles = useQuery({
    queryKey: ["local", local.path, preferences.hidden],
    queryFn: () =>
      api<FileEntry[]>("local_list", {
        path: local.path,
        hidden: preferences.hidden,
      }),
    enabled: desktop && !!local.path,
  });
  const remoteFiles = useInfiniteQuery({
    queryKey: ["remote", profile?.id, remote.path],
    enabled: desktop && !!profile,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      api<Page>("remote_list", {
        id: profile?.id,
        prefix: remote.path,
        token: pageParam,
      }),
    getNextPageParam: (page) => page.next_token,
  });
  const transfers = useQuery({
    queryKey: ["transfers"],
    queryFn: () => api<Transfer[]>("transfers"),
    enabled: desktop,
    refetchInterval: 750,
  });
  const entries = remoteFiles.data?.pages.flatMap((p) => p.entries) ?? [];
  const jobs = transfers.data ?? [];
  const active = jobs.filter((j) =>
    ["uploading", "downloading", "queued", "preparing", "verifying"].includes(
      j.status,
    ),
  ).length;
  useEffect(() => {
    document.documentElement.dataset.theme = preferences.theme;
  }, [preferences.theme]);
  async function run(work: () => Promise<void>) {
    setError("");
    try {
      await work();
    } catch (e) {
      setError(message(e));
    }
  }
  async function refresh() {
    await Promise.all([
      query.invalidateQueries({ queryKey: ["local"] }),
      query.invalidateQueries({ queryKey: ["remote"] }),
      query.invalidateQueries({ queryKey: ["transfers"] }),
    ]);
  }
  function connect(p: Profile, prefix = p.prefix) {
    setConnectionId(p.id);
    setRemoteStart(p.prefix);
    remote.go(prefix);
    setRemoteSelection([]);
  }
  async function upload(
    paths?: string[],
    folder = false,
    prefix = remote.path,
  ) {
    if (!profile) {
      setConnectionForm("new");
      return;
    }
    const picked =
      paths ??
      (await openDialog({
        multiple: !folder,
        directory: folder,
        title: folder ? "Select folder to upload" : "Select files to upload",
      }));
    if (!picked) return;
    await api("upload", {
      id: profile.id,
      paths: typeof picked === "string" ? [picked] : picked,
      prefix,
    });
    preferences.set({ queueOpen: true });
  }
  async function download(keys?: string[], directory = local.path) {
    if (!profile) return;
    const chosen = keys ?? remoteSelection;
    if (!chosen.length) {
      setError("Select remote files to download.");
      return;
    }
    if (!directory)
      throw new Error("Choose a local destination directory first.");
    await api("download", {
      id: profile.id,
      keys: chosen,
      directory,
    });
    preferences.set({ queueOpen: true });
  }
  async function showProperties(entry: FileEntry, isRemote: boolean) {
    const properties =
      isRemote && profile
        ? await api<Record<string, unknown>>("properties", {
            id: profile.id,
            key: entry.path,
          })
        : entry;
    setPrompt({
      title: entry.name,
      description: entry.path,
      content: (
        <pre className="properties">{JSON.stringify(properties, null, 2)}</pre>
      ),
      execute: async () => {
        if (isRemote && profile) {
          const text = await api<string>("preview", {
            id: profile.id,
            key: entry.path,
          });
          setPrompt({
            title: entry.name,
            description: "UTF-8 text · maximum 512 KiB · read-only",
            content: <pre className="text-preview">{text}</pre>,
            label: "Done",
            execute: async () => {},
          });
        }
      },
      label: isRemote ? "Load text preview" : "Done",
      keepOpen: isRemote,
    });
  }
  function deleteSelected(isRemote: boolean, selected: FileEntry[]) {
    if (!selected.length) return;
    if (
      isRemote &&
      (profile?.read_only || selected.some((entry) => entry.is_dir))
    ) {
      setError("Select remote files from a writable connection to delete.");
      return;
    }
    const id = isRemote ? profile?.id : null;
    if (isRemote && !id) return;
    setPrompt({
      title: isRemote
        ? `Delete ${selected.length} file(s)?`
        : `Recycle ${selected.length} item(s)?`,
      description: isRemote
        ? "Deletes the selected current objects. Provider versioning may retain older versions. Changed objects are protected by their listed ETags."
        : "Moves the selected local items to the operating system recycle bin.",
      destructive: true,
      label: isRemote ? "Delete files" : "Move to recycle bin",
      content: (
        <div className="delete-preview">
          {selected.slice(0, 10).map((entry) => (
            <code key={entry.path}>{entry.path}</code>
          ))}
          {selected.length > 10 && <span>and {selected.length - 10} more</span>}
        </div>
      ),
      execute: async () => {
        const result = await api<DeleteResult>("delete_entries", {
          id,
          entries: selected.map((entry) => ({
            path: entry.path,
            etag: entry.etag,
          })),
        });
        const deleted = new Set(result.deleted);
        (isRemote ? setRemoteSelection : setLocalSelection)((previous) =>
          previous.filter((path) => !deleted.has(path)),
        );
        await refresh();
        setNotice(
          `${result.deleted.length} item(s) ${isRemote ? "deleted" : "moved to recycle bin"}.`,
        );
        if (result.failed)
          setError(
            `${result.failed} item(s) could not be deleted. ${result.errors.join("; ")}`,
          );
      },
    });
  }
  async function controlAll(operation: BatchOperation) {
    setBulkBusy(true);
    try {
      const result = await api<BatchControlResult>("transfer_control_all", {
        operation,
      });
      await query.invalidateQueries({ queryKey: ["transfers"] });
      setNotice(
        `${operation === "pause" ? "Pause" : operation === "resume" ? "Resume" : "Cancellation"} requested for ${result.requested} transfer(s). In-flight requests may finish before stopping.`,
      );
      if (result.failed)
        throw new Error(
          `${result.failed} transfer(s) need attention: ${result.errors.join("; ")}`,
        );
    } finally {
      setBulkBusy(false);
    }
  }
  function bulkControl(operation: BatchOperation) {
    if (operation !== "cancel") {
      void run(() => controlAll(operation));
      return;
    }
    setPrompt({
      title: "Cancel all transfers?",
      description:
        "Cancels running, queued, paused, and failed transfers across every connection. Completed files remain. Multipart uploads and temporary download files are cleaned up where possible.",
      destructive: true,
      label: "Cancel all transfers",
      execute: () => controlAll("cancel"),
    });
  }
  function action(isRemote: boolean, name: string, entry?: FileEntry) {
    const base = isRemote ? remote.path : local.path;
    if (name === "connect") {
      setConnectionForm("new");
      return;
    }
    if (name === "delete-selected") {
      const paths = new Set(isRemote ? remoteSelection : localSelection);
      deleteSelected(
        isRemote,
        (isRemote ? entries : (localFiles.data ?? [])).filter((item) =>
          paths.has(item.path),
        ),
      );
      return;
    }
    if (name === "mkdir") {
      setPrompt({
        title: "New folder",
        description: isRemote
          ? "Creates an S3 folder marker at the current prefix."
          : "Creates a directory at the current local path.",
        input: "",
        execute: async (value) => {
          if (
            !value.trim() ||
            /[/\\]/.test(value) ||
            value === "." ||
            value === ".."
          )
            throw new Error("Enter a single folder name.");
          if (isRemote && profile)
            await api("remote_mkdir", {
              id: profile.id,
              key: `${base}${value}/`,
            });
          else await api("local_mkdir", { path: base, name: value });
          await refresh();
        },
      });
      return;
    }
    if (!entry) return;
    if (name === "transfer") {
      void run(() =>
        isRemote ? download([entry.path]) : upload([entry.path]),
      );
      return;
    }
    if (name === "copy-path") {
      void run(async () => {
        await navigator.clipboard.writeText(entry.path);
        setNotice("Path copied to clipboard");
      });
      return;
    }
    if (name === "presign" && profile) {
      void run(async () => {
        const url = await api<string>("presign", {
          id: profile.id,
          key: entry.path,
          seconds: 3600,
        });
        await navigator.clipboard.writeText(url);
        setNotice(
          "Signed URL copied · expires in 1 hour · grants object access",
        );
      });
      return;
    }
    if (name === "properties") {
      void run(() => showProperties(entry, isRemote));
      return;
    }
    if (name === "delete") {
      deleteSelected(isRemote, [entry]);
      return;
    }
    if (name === "rename" || name === "copy")
      setPrompt({
        title: isRemote ? "Copy remote object" : "Rename local item",
        description: isRemote
          ? "Enter the complete destination key. S3 copy preserves the source; destination absence is checked before copying but is not atomic."
          : "The destination must not exist.",
        input: entry.name,
        execute: async (value) => {
          if (isRemote && profile)
            await api("remote_copy", {
              id: profile.id,
              source: entry.path,
              destination: value,
              etag: entry.etag,
            });
          else await api("local_rename", { path: entry.path, name: value });
          await refresh();
        },
      });
  }
  const readonly = !!profile?.read_only;
  const focusedRemote = focusedSide === "remote";
  const focusedSelection = focusedRemote ? remoteSelection : localSelection;
  const focusedPaths = new Set(focusedSelection);
  const focusedEntries = (
    focusedRemote ? entries : (localFiles.data ?? [])
  ).filter((entry) => focusedPaths.has(entry.path));
  const canDelete =
    desktop &&
    focusedEntries.length > 0 &&
    focusedEntries.length === focusedSelection.length &&
    (!focusedRemote ||
      (!!profile &&
        !readonly &&
        focusedEntries.every((entry) => !entry.is_dir)));
  const signedEntry =
    remoteSelection.length === 1
      ? entries.find(
          (entry) => entry.path === remoteSelection[0] && !entry.is_dir,
        )
      : undefined;
  const connected = !!profile && remoteFiles.isSuccess;
  const currentError =
    error ||
    [profiles.error, home.error, localFiles.error, remoteFiles.error]
      .filter(Boolean)
      .map(message)
      .join("\n");
  const drag = useFileDragDrop({
    canUpload: desktop && !!profile && !readonly,
    canDownload: desktop && !!profile && !!local.path,
    onSelect: (side, paths) =>
      (side === "local" ? setLocalSelection : setRemoteSelection)(paths),
    onTransfer: (source, paths, target) =>
      void run(() =>
        source === "local"
          ? upload(paths, false, target.path)
          : download(paths, target.path),
      ),
    onError: setError,
    onExternalDrop: (paths, target) => {
      if (target?.side !== "remote") {
        setError(
          "Drop files onto the remote pane or a remote folder to upload.",
        );
        return;
      }
      if (!profile) {
        setConnectionForm("new");
        setError("Connect a bucket before dropping files.");
        return;
      }
      if (readonly) {
        setError("This connection is read-only.");
        return;
      }
      if (!paths.length) return;
      setPrompt({
        title: `Upload ${paths.length} dropped item(s)?`,
        description: `Destination: ${profile.bucket}/${target.path}. Existing objects will not be overwritten.`,
        label: "Upload items",
        execute: () => upload(paths, false, target.path),
      });
    },
  });
  function showView(view: "files" | "transfers") {
    setMainView(view);
    setNavigationOpen(false);
    if (view === "transfers") preferences.set({ queueOpen: true });
  }
  function bookmarkPair() {
    if (!profile || !local.path) return;
    preferences.set({
      bookmarks: [
        ...preferences.bookmarks,
        {
          label: `${profile.bucket}/${remote.path}`,
          profile: profile.id,
          prefix: remote.path,
          local: local.path,
        },
      ],
    });
  }
  return (
    <div
      className="app-shell"
      onKeyDown={(e) => {
        if (e.key === "Escape" && navigationOpen) {
          setNavigationOpen(false);
          document.getElementById("navigation-toggle")?.focus();
        }
      }}
    >
      <WorkspaceHeader
        view={mainView}
        onView={showView}
        onBookmarks={() => setBookmarksOpen(true)}
        onActivity={() => setActivityOpen(true)}
        attention={
          !!(
            currentError ||
            notice ||
            jobs.some((job) => job.status === "failed")
          )
        }
        search={search}
        onSearch={setSearch}
        navigationOpen={navigationOpen}
        onNavigation={() => setNavigationOpen((open) => !open)}
        layout={preferences.view}
        onLayout={(view) => preferences.set({ view })}
      />
      <div className="workspace">
        <aside
          id="workspace-navigation"
          aria-label="Workspace navigation"
          className={`sidebar ${navigationOpen ? "open" : ""}`}
        >
          <button
            className={`sidebar-link ${mainView === "files" ? "selected" : ""}`}
            onClick={() => showView("files")}
          >
            <HardDrive size={17} /> Explorer
          </button>
          <button
            className={`sidebar-link ${mainView === "transfers" ? "selected" : ""}`}
            onClick={() => showView("transfers")}
          >
            <ArrowDownToLine size={16} /> Transfers
            {active > 0 && <span className="count">{active}</span>}
          </button>
          <button
            className="sidebar-link"
            onClick={() => setConnectionForm(profile ?? "new")}
          >
            <ShieldCheck size={16} /> Credentials
          </button>
          <div className="sidebar-section">
            <span>Connections</span>
            <button
              aria-label="Add connection"
              onClick={() => setConnectionForm("new")}
            >
              <Plus size={15} />
            </button>
          </div>
          <div className="connection-list">
            {profiles.data?.map((p) => (
              <div
                className={`connection-item ${profile?.id === p.id ? "active" : ""}`}
                key={p.id}
              >
                <button
                  onClick={() => {
                    connect(p);
                    showView("files");
                  }}
                >
                  <span
                    className={`connection-dot ${profile?.id === p.id && remoteFiles.isSuccess ? "online" : ""}`}
                  />
                  <span>
                    <strong>{p.name}</strong>
                  </span>
                </button>
                <Dropdown.Root>
                  <Dropdown.Trigger asChild>
                    <button aria-label={`Manage ${p.name}`}>
                      <MoreHorizontal size={14} />
                    </button>
                  </Dropdown.Trigger>
                  <Dropdown.Portal>
                    <Dropdown.Content className="dropdown">
                      <Dropdown.Item onSelect={() => setConnectionForm(p)}>
                        Edit connection
                      </Dropdown.Item>
                      <Dropdown.Item
                        onSelect={() =>
                          setConnectionForm({
                            ...p,
                            id: crypto.randomUUID(),
                            name: `${p.name} copy`,
                          })
                        }
                      >
                        Duplicate (re-enter credentials)
                      </Dropdown.Item>
                      <Dropdown.Item
                        onSelect={() =>
                          void run(async () => {
                            await navigator.clipboard.writeText(
                              JSON.stringify(p, null, 2),
                            );
                            setNotice("Profile copied without credentials");
                          })
                        }
                      >
                        <Copy size={13} /> Export without secrets
                      </Dropdown.Item>
                      <Dropdown.Item
                        onSelect={() =>
                          void run(async () => {
                            await api("test_connection", { id: p.id });
                            setNotice("Bucket access verified");
                          })
                        }
                      >
                        Test bucket access
                      </Dropdown.Item>
                      <Dropdown.Item
                        className="destructive"
                        onSelect={() =>
                          setPrompt({
                            title: "Delete connection?",
                            description: `Remove ${p.name} and its OS keychain credentials. Bucket objects are preserved.`,
                            destructive: true,
                            execute: async () => {
                              await api("delete_profile", { id: p.id });
                              await query.invalidateQueries({
                                queryKey: ["profiles"],
                              });
                              if (connectionId === p.id) setConnectionId("");
                            },
                          })
                        }
                      >
                        Delete connection…
                      </Dropdown.Item>
                    </Dropdown.Content>
                  </Dropdown.Portal>
                </Dropdown.Root>
                {profile?.id === p.id && (
                  <button
                    className="connection-bucket"
                    onClick={() => {
                      connect(p);
                      showView("files");
                    }}
                  >
                    <Archive size={14} />
                    <span>{p.bucket}</span>
                  </button>
                )}
              </div>
            ))}
          </div>
          {!profiles.data?.length && (
            <div className="sidebar-empty">
              <CloudOff size={20} />
              <p>No connections yet</p>
            </div>
          )}
          <button
            className="add-connection"
            onClick={() => setConnectionForm("new")}
          >
            <Plus size={14} /> New connection
          </button>
          <div className="sidebar-bottom">
            <button className="sidebar-link" onClick={() => setSettings(true)}>
              <Settings2 size={16} /> Settings
            </button>
            <span className="version">v{version}</span>
          </div>
        </aside>
        <main
          className={`workspace-content ${mainView === "transfers" ? "transfers-view" : ""}`}
        >
          {!desktop && (
            <div className="browser-notice">
              <Terminal size={15} />
              <span>
                <strong>Desktop preview</strong>
                <span>
                  Launch with <code>npm run desktop dev</code> for filesystem
                  and S3 access.
                </span>
              </span>
            </div>
          )}
          {mainView === "files" && (
            <div className="commandbar" aria-label="File commands">
              <div className="command-transfers">
                <button
                  disabled={!desktop || !profile || readonly}
                  onClick={() =>
                    void run(() =>
                      upload(
                        localSelection.length ? localSelection : undefined,
                      ),
                    )
                  }
                >
                  <span className="action-icon upload">
                    <ArrowUpFromLine size={16} />
                  </span>
                  <span>
                    <strong>Upload</strong>
                    <small>Local → S3</small>
                  </span>
                </button>
                <button
                  disabled={
                    !desktop ||
                    !profile ||
                    !remoteSelection.length ||
                    !local.path
                  }
                  onClick={() => void run(() => download())}
                >
                  <span className="action-icon download">
                    <ArrowDownToLine size={16} />
                  </span>
                  <span>
                    <strong>Download</strong>
                    <small>S3 → Local</small>
                  </span>
                </button>
              </div>
              <span className="divider" />
              <Button
                variant="ghost"
                disabled={
                  !desktop ||
                  (focusedRemote ? !profile || readonly : !local.path)
                }
                title={`Create a ${focusedSide} folder`}
                aria-label="New folder"
                onClick={() => action(focusedRemote, "mkdir")}
              >
                <FolderPlus size={15} />
                <span>New folder</span>
              </Button>
              <Button
                variant="ghost"
                disabled={!signedEntry}
                title="Copy a signed URL for one selected remote file; expires in 1 hour"
                aria-label="Copy signed URL"
                onClick={() =>
                  signedEntry && action(true, "presign", signedEntry)
                }
              >
                <Copy size={15} />
                <span>Copy URL</span>
              </Button>
              <Button
                variant="ghost"
                disabled={!canDelete}
                aria-label="Delete selected items"
                title="Delete selected files from the focused pane"
                onClick={() => deleteSelected(focusedRemote, focusedEntries)}
              >
                <Trash2 size={15} />
                <span>
                  Delete
                  {focusedEntries.length ? ` (${focusedEntries.length})` : ""}
                </span>
              </Button>
              <Button
                variant="ghost"
                disabled={!profile || !local.path}
                title="Preview synchronization"
                aria-label="Synchronize"
                onClick={() => setSync(true)}
              >
                <Workflow size={15} />
                <span>Synchronize</span>
              </Button>
              <div className="connection-health">
                <span
                  className={`connection-dot ${connected ? "online" : ""}`}
                />
                <div>
                  <strong>
                    {connected
                      ? "Connected"
                      : remoteFiles.isError
                        ? "Connection error"
                        : profile
                          ? "Connecting…"
                          : "Disconnected"}
                  </strong>
                  <small>
                    {profile
                      ? profile.provider.toUpperCase()
                      : "Select a connection"}
                  </small>
                </div>
              </div>
            </div>
          )}
          {(error || profiles.error || home.error) && (
            <div className="global-alert" role="alert">
              <span>{error || message(profiles.error || home.error)}</span>
              <button onClick={() => setError("")} aria-label="Dismiss error">
                <X size={15} />
              </button>
            </div>
          )}
          {notice && (
            <div className="global-notice" role="status">
              <Check size={14} />
              <span>{notice}</span>
              <button
                onClick={() => setNotice("")}
                aria-label="Dismiss notification"
              >
                <X size={14} />
              </button>
            </div>
          )}
          <div
            className={`panes ${preferences.view}`}
            hidden={mainView !== "files"}
          >
            {preferences.view !== "remote" && (
              <section
                className={`pane local-pane ${focusedSide === "local" ? "focused-pane" : ""}`}
                data-drop-side="local"
                data-drop-path={local.path}
                data-drop-active={drag.target?.side === "local" || undefined}
                data-drop-hint={
                  drag.target?.side === "local"
                    ? `Drop to download: ${drag.target.path || "bucket root"}`
                    : undefined
                }
                onFocusCapture={() => setFocusedSide("local")}
                onPointerDownCapture={(event) => {
                  setFocusedSide("local");
                  drag.begin(event, "local", localSelection);
                }}
              >
                <div className="pane-heading">
                  <div className="pane-source">
                    <span className="source-mark local">
                      <HardDrive size={16} />
                    </span>
                    <div>
                      <span className="eyebrow">LOCAL</span>
                      <strong>My Computer</strong>
                    </div>
                  </div>
                  <div className="pane-header-actions">
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="Refresh local listing"
                      disabled={!desktop}
                      onClick={() => void localFiles.refetch()}
                    >
                      <RefreshCw size={14} />
                    </Button>
                    <Button
                      variant="ghost"
                      size="small"
                      disabled={!desktop}
                      onClick={() =>
                        void run(async () => {
                          const path = await openDialog({
                            directory: true,
                            title: "Open local folder",
                          });
                          if (typeof path === "string") {
                            local.go(path);
                            setLocalSelection([]);
                          }
                        })
                      }
                    >
                      <FolderOpen size={14} /> Browse
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="New local folder"
                      disabled={!desktop || !local.path}
                      onClick={() => action(false, "mkdir")}
                    >
                      <FolderPlus size={14} />
                    </Button>
                  </div>
                </div>
                <PathBar
                  path={local.path}
                  label="Local directory"
                  back={local.back}
                  forward={local.forward}
                  canBack={local.canBack}
                  canForward={local.canForward}
                  up={() => {
                    local.go(parent(local.path));
                    setLocalSelection([]);
                  }}
                  navigate={(path) => {
                    local.go(path);
                    setLocalSelection([]);
                  }}
                  disabled={!desktop}
                />
                <Files
                  canTransfer={desktop && !!profile && !readonly}
                  dropPath={
                    drag.target?.side === "local" ? drag.target.path : undefined
                  }
                  filter={search}
                  showFilter={false}
                  entries={localFiles.data ?? []}
                  selected={localSelection}
                  onSelect={setLocalSelection}
                  onOpen={(entry) => {
                    if (entry.is_dir) {
                      local.go(entry.path);
                      setLocalSelection([]);
                    } else void run(() => showProperties(entry, false));
                  }}
                  loading={localFiles.isFetching}
                  error={
                    localFiles.error ? message(localFiles.error) : undefined
                  }
                  connected={desktop}
                  onAction={(name, entry) => action(false, name, entry)}
                  onRetry={() => void localFiles.refetch()}
                />
              </section>
            )}
            {preferences.view === "dual" && <SplitHandle />}
            {preferences.view !== "local" && (
              <section
                className={`pane remote-pane ${focusedSide === "remote" ? "focused-pane" : ""}`}
                data-drop-side="remote"
                data-drop-path={remote.path}
                data-drop-active={drag.target?.side === "remote" || undefined}
                data-drop-hint={
                  drag.target?.side === "remote"
                    ? `Drop to upload: ${drag.target.path || "bucket root"}`
                    : undefined
                }
                onFocusCapture={() => setFocusedSide("remote")}
                onPointerDownCapture={(event) => {
                  setFocusedSide("remote");
                  drag.begin(event, "remote", remoteSelection);
                }}
              >
                <div className="pane-heading">
                  <div className="pane-source">
                    <span className="source-mark remote">
                      <Cloud size={16} />
                    </span>
                    <div>
                      <span className="eyebrow">REMOTE</span>
                      <select
                        aria-label="Active connection"
                        value={profile?.id ?? ""}
                        onChange={(e) => {
                          const selected = profiles.data?.find(
                            (p) => p.id === e.target.value,
                          );
                          if (selected) connect(selected);
                        }}
                      >
                        <option value="">Select a connection</option>
                        {profiles.data?.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.name} / {p.bucket}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                  <div className="pane-header-actions">
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="Refresh remote listing"
                      disabled={!profile}
                      onClick={() => void remoteFiles.refetch()}
                    >
                      <RefreshCw size={14} />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="New remote folder"
                      disabled={!profile || readonly}
                      onClick={() => action(true, "mkdir")}
                    >
                      <FolderPlus size={14} />
                    </Button>
                    <Dropdown.Root>
                      <Dropdown.Trigger asChild>
                        <Button
                          size="icon"
                          variant="ghost"
                          disabled={!profile || readonly}
                          aria-label="Remote pane actions"
                        >
                          <MoreHorizontal size={16} />
                        </Button>
                      </Dropdown.Trigger>
                      <Dropdown.Portal>
                        <Dropdown.Content className="dropdown">
                          <Dropdown.Item
                            onSelect={() =>
                              void run(() => upload(undefined, true))
                            }
                          >
                            Upload folder recursively…
                          </Dropdown.Item>
                        </Dropdown.Content>
                      </Dropdown.Portal>
                    </Dropdown.Root>
                  </div>
                </div>
                <PathBar
                  path={remote.path}
                  label="Remote prefix"
                  back={remote.back}
                  forward={remote.forward}
                  canBack={remote.canBack}
                  canForward={remote.canForward}
                  up={() => {
                    remote.go(parent(remote.path, true));
                    setRemoteSelection([]);
                  }}
                  navigate={(path) => {
                    remote.go(path && !path.endsWith("/") ? `${path}/` : path);
                    setRemoteSelection([]);
                  }}
                  disabled={!profile}
                  bucket={profile?.bucket}
                />
                <Files
                  remote
                  canTransfer={desktop && !!profile}
                  dropPath={
                    drag.target?.side === "remote"
                      ? drag.target.path
                      : undefined
                  }
                  filter={search}
                  showFilter={false}
                  entries={entries}
                  selected={remoteSelection}
                  onSelect={setRemoteSelection}
                  onOpen={(entry) => {
                    if (entry.is_dir) {
                      remote.go(entry.path);
                      setRemoteSelection([]);
                    } else void run(() => showProperties(entry, true));
                  }}
                  loading={remoteFiles.isFetching && !entries.length}
                  error={
                    remoteFiles.error ? message(remoteFiles.error) : undefined
                  }
                  connected={!!profile}
                  writable={!readonly}
                  onAction={(name, entry) => action(true, name, entry)}
                  onRetry={() => void remoteFiles.refetch()}
                />
                {remoteFiles.hasNextPage && (
                  <Button
                    className="load-more"
                    disabled={remoteFiles.isFetchingNextPage}
                    onClick={() => void remoteFiles.fetchNextPage()}
                  >
                    {remoteFiles.isFetchingNextPage
                      ? "Loading…"
                      : "Load next 1,000 objects"}
                  </Button>
                )}
              </section>
            )}
          </div>
          <Queue
            jobs={jobs}
            bulkControl={bulkControl}
            bulkBusy={bulkBusy}
            open={mainView === "transfers" || preferences.queueOpen}
            toggle={() => {
              if (mainView === "transfers") {
                showView("files");
                preferences.set({ queueOpen: false });
              } else preferences.set({ queueOpen: !preferences.queueOpen });
            }}
            control={(id, operation) =>
              void run(async () => {
                await api("transfer_control", { id, operation });
                await query.invalidateQueries({ queryKey: ["transfers"] });
              })
            }
            clear={() =>
              void run(async () => {
                await api("clear_finished");
                await refresh();
              })
            }
          />
        </main>
      </div>
      <footer className="statusbar">
        <span>
          {profile ? (
            <>
              <span className={`tiny-dot ${connected ? "online" : ""}`} />
              {profile.bucket}
            </>
          ) : (
            <>
              <Unplug size={11} /> Disconnected
            </>
          )}
        </span>
        <span>
          {active ? `${active} active transfers` : "No active transfers"}
          <span className="status-separator">/</span>
          {bytes(jobs.reduce((s, j) => s + j.bytes_done, 0))} confirmed
          <span className="status-separator">/</span>
          <LockKeyhole size={10} /> Credentials in OS keychain
        </span>
      </footer>
      {drag.dragging && (
        <div
          ref={drag.ghost}
          className="file-drag-ghost"
          style={{
            transform: `translate3d(${drag.dragging.x + 14}px,${drag.dragging.y + 14}px,0)`,
          }}
        >
          {drag.dragging.side === "local" ? (
            <ArrowUpFromLine size={15} />
          ) : (
            <ArrowDownToLine size={15} />
          )}
          {drag.dragging.paths.length} item(s)
          <small>
            Drop on the {drag.dragging.side === "local" ? "remote" : "local"}{" "}
            pane or a folder
          </small>
        </div>
      )}
      {bookmarksOpen && (
        <Modal
          title="Bookmarks"
          description="Saved local folders paired with remote prefixes."
          onClose={() => setBookmarksOpen(false)}
        >
          <div className="form">
            <Button disabled={!profile || !local.path} onClick={bookmarkPair}>
              <Bookmark size={15} /> Bookmark current pair
            </Button>
            {!preferences.bookmarks.length && (
              <p className="muted">
                No bookmarks yet. Connect a bucket and choose a local folder to
                save a pair.
              </p>
            )}
            {preferences.bookmarks.map((bookmark, index) => (
              <div
                className="bookmark-row"
                key={`${bookmark.profile}-${index}`}
              >
                <button
                  onClick={() => {
                    const saved = profiles.data?.find(
                      (p) => p.id === bookmark.profile,
                    );
                    if (saved) {
                      connect(saved, bookmark.prefix);
                      local.go(bookmark.local);
                      showView("files");
                      setBookmarksOpen(false);
                    } else
                      setError(
                        "The connection for this bookmark no longer exists.",
                      );
                  }}
                >
                  <Bookmark size={15} />
                  <span>
                    {bookmark.label}
                    <small>{bookmark.local}</small>
                  </span>
                </button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove bookmark ${bookmark.label}`}
                  onClick={() =>
                    preferences.set({
                      bookmarks: preferences.bookmarks.filter(
                        (_, i) => i !== index,
                      ),
                    })
                  }
                >
                  <X size={14} />
                </Button>
              </div>
            ))}
          </div>
        </Modal>
      )}
      {activityOpen && (
        <Modal
          title="Activity"
          description="Current connection and transfer status."
          onClose={() => setActivityOpen(false)}
        >
          <div className="form">
            {currentError && <p className="error-box">{currentError}</p>}
            {notice && <p className="notice">{notice}</p>}
            {jobs
              .filter((job) => job.status === "failed")
              .map((job) => (
                <p className="error-box" key={job.id}>
                  {job.key}: {job.error || "Transfer failed"}
                </p>
              ))}
            {!currentError &&
              !notice &&
              !jobs.some((job) => job.status === "failed") && (
                <p className="muted">No current alerts.</p>
              )}
            <Button
              onClick={() => {
                showView("transfers");
                setActivityOpen(false);
              }}
            >
              Open transfer queue
            </Button>
          </div>
        </Modal>
      )}
      {connectionForm && (
        <ConnectionDialog
          initial={connectionForm === "new" ? undefined : connectionForm}
          onClose={() => setConnectionForm(undefined)}
          onSaved={connect}
        />
      )}
      {settings && <SettingsDialog onClose={() => setSettings(false)} />}
      {sync && profile && (
        <SyncDialog
          profile={profile}
          local={local.path}
          prefix={remote.path}
          onClose={() => setSync(false)}
          onApplied={() => {
            preferences.set({ queueOpen: true });
            void refresh();
          }}
        />
      )}
      {prompt && (
        <ActionPrompt prompt={prompt} onClose={() => setPrompt(undefined)} />
      )}
    </div>
  );
}

function PathBar({
  path,
  label,
  back,
  forward,
  canBack,
  canForward,
  up,
  navigate,
  disabled,
  bucket,
}: {
  path: string;
  label: string;
  back: () => void;
  forward: () => void;
  canBack: boolean;
  canForward: boolean;
  up: () => void;
  navigate: (path: string) => void;
  disabled: boolean;
  bucket?: string;
}) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState("");
  return (
    <div className="pathbar">
      <Button
        size="icon"
        variant="ghost"
        aria-label={`Back in ${label}`}
        disabled={!canBack || disabled}
        onClick={back}
      >
        <ArrowLeft size={14} />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        aria-label={`Forward in ${label}`}
        disabled={!canForward || disabled}
        onClick={forward}
      >
        <ArrowRight size={14} />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        aria-label={`Parent ${label}`}
        disabled={disabled || !path}
        onClick={up}
      >
        <ArrowUp size={14} />
      </Button>
      <span className="path-separator" />
      {editing ? (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            navigate(value);
            setEditing(false);
          }}
        >
          <input
            autoFocus
            aria-label={label}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onBlur={() => setEditing(false)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setEditing(false);
            }}
          />
        </form>
      ) : (
        <button
          className="path-display mono"
          aria-label={`Edit ${label}`}
          disabled={disabled}
          onClick={() => {
            setValue(path);
            setEditing(true);
          }}
        >
          {bucket && (
            <>
              <span className="bucket-name">{bucket}</span>
              <span className="breadcrumb-slash">/</span>
            </>
          )}
          {path || (bucket ? "" : "Choose a folder")}
          <ChevronDown size={12} />
        </button>
      )}
    </div>
  );
}
function ActionPrompt({
  prompt,
  onClose,
}: {
  prompt: Prompt;
  onClose: () => void;
}) {
  const [value, setValue] = useState(prompt.input ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Modal
      title={prompt.title}
      description={prompt.description}
      onClose={onClose}
      wide={!!prompt.content}
    >
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          setBusy(true);
          setError("");
          void prompt
            .execute(value)
            .then(() => {
              if (!prompt.keepOpen) onClose();
            })
            .catch((e: unknown) => setError(message(e)))
            .finally(() => setBusy(false));
        }}
      >
        {prompt.input !== undefined && (
          <label>
            Name / destination
            <input
              autoFocus
              required
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </label>
        )}
        {prompt.content}
        {error && (
          <p role="alert" className="error-box">
            {error}
          </p>
        )}
        <div className="modal-footer">
          <Button onClick={onClose}>Close</Button>
          <Button
            type="submit"
            variant={prompt.destructive ? "destructive" : "default"}
            disabled={busy}
          >
            {busy ? "Working…" : (prompt.label ?? "Continue")}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
