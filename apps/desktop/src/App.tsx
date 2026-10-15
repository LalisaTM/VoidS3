import { useEffect, useState, type ReactNode } from "react";
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ArrowDownToLine,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  ArrowUpFromLine,
  Bookmark,
  Check,
  ChevronDown,
  Cloud,
  CloudOff,
  Copy,
  FolderOpen,
  HardDrive,
  LayoutPanelLeft,
  LockKeyhole,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
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
  type Page,
  type Profile,
  type Transfer,
} from "./lib/api";
import { usePreferences } from "./store";
import { version } from "../package.json";
import { Button } from "./components/ui/button";
import { ConnectionDialog } from "./components/ConnectionDialog";
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
  execute: (value: string) => Promise<void>;
}
export default function App() {
  const query = useQueryClient();
  const preferences = usePreferences();
  const setPreferences = preferences.set;
  const [connectionId, setConnectionId] = useState("");
  const [connectionForm, setConnectionForm] = useState<Profile | "new">();
  const [settings, setSettings] = useState(false);
  const [sync, setSync] = useState(false);
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
  useEffect(() => {
    if (!desktop) return;
    let cleanup: (() => void) | undefined;
    let disposed = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && profile && !profile.read_only) {
          const paths = event.payload.paths;
          setPrompt({
            title: `Upload ${paths.length} dropped item(s)?`,
            description: `Destination: ${profile.bucket}/${remote.path}. Existing remote objects will be preserved.`,
            execute: async () => {
              await api("upload", {
                id: profile.id,
                paths,
                prefix: remote.path,
              });
              setPreferences({ queueOpen: true });
            },
          });
        }
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else cleanup = unlisten;
      })
      .catch((e: unknown) => setError(message(e)));
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [profile, remote.path, setPreferences]);

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
  function connect(p: Profile) {
    setConnectionId(p.id);
    setRemoteStart(p.prefix);
    remote.go(p.prefix);
    setRemoteSelection([]);
  }
  async function upload(paths?: string[], folder = false) {
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
      prefix: remote.path,
    });
    preferences.set({ queueOpen: true });
  }
  async function download(keys?: string[]) {
    if (!profile) return;
    const chosen = keys ?? remoteSelection;
    if (!chosen.length) {
      setError("Select remote files to download.");
      return;
    }
    if (!local.path)
      throw new Error("Choose a local destination directory first.");
    await api("download", {
      id: profile.id,
      keys: chosen,
      directory: local.path,
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
            execute: async () => {},
          });
        }
      },
      label: isRemote ? "Load text preview" : "Done",
    });
  }
  function action(isRemote: boolean, name: string, entry?: FileEntry) {
    const base = isRemote ? remote.path : local.path;
    if (name === "connect") {
      setConnectionForm("new");
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
      setPrompt({
        title: isRemote ? "Delete this object?" : "Move to recycle bin?",
        description: `${entry.path}${isRemote ? ". This removes the current object. Provider versioning may retain older versions." : ". The operating system handles recycling."}`,
        destructive: true,
        label: isRemote ? "Delete object" : "Recycle",
        execute: async () => {
          if (isRemote && profile)
            await api("remote_delete", {
              id: profile.id,
              key: entry.path,
              etag: entry.etag,
            });
          else await api("local_delete", { path: entry.path });
          await refresh();
        },
      });
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
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <svg viewBox="0 0 32 32" width="29" height="29" aria-hidden="true">
            <path d="M3 5 16 28 29 5h-8l-5 10-5-10Z" fill="currentColor" />
          </svg>
          <span>
            VoidS3<span className="brand-period">.</span>
          </span>
          <span className="version">{version}</span>
        </div>
        <div className="workspace-label">
          YOUR WORKSPACE <LockKeyhole size={11} />
        </div>
        <button
          className="sidebar-link selected"
          onClick={() => preferences.set({ view: "dual" })}
        >
          <LayoutPanelLeft size={17} /> File explorer
        </button>
        <button
          className="sidebar-link"
          onClick={() => preferences.set({ queueOpen: !preferences.queueOpen })}
        >
          <ArrowDownToLine size={16} /> Transfers
          {active > 0 && <span className="count">{active}</span>}
        </button>
        <div className="sidebar-section">
          <span>CONNECTIONS</span>
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
              <button onClick={() => connect(p)}>
                <span
                  className={`connection-dot ${profile?.id === p.id && remoteFiles.isSuccess ? "online" : ""}`}
                />
                <span>
                  <strong>{p.name}</strong>
                  <small>
                    {p.provider === "r2"
                      ? "Cloudflare R2"
                      : p.provider.toUpperCase()}
                  </small>
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
            </div>
          ))}
        </div>
        {!profiles.data?.length && (
          <div className="sidebar-empty">
            <CloudOff size={20} />
            <p>No connections yet</p>
            <button onClick={() => setConnectionForm("new")}>
              Connect your first bucket <ArrowRight size={12} />
            </button>
          </div>
        )}
        <button
          className="add-connection"
          onClick={() => setConnectionForm("new")}
        >
          <Plus size={14} /> New connection
        </button>
        <div className="sidebar-section">
          <span>BOOKMARKS</span>
          <button
            aria-label="Bookmark current pair"
            disabled={!profile || !local.path}
            onClick={() => {
              if (profile)
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
            }}
          >
            <Plus size={15} />
          </button>
        </div>
        {!preferences.bookmarks.length && (
          <p className="sidebar-hint">
            Keep a local folder and remote prefix together.
          </p>
        )}
        {preferences.bookmarks.map((b, i) => (
          <div className="bookmark-row" key={`${b.profile}-${i}`}>
            <button
              onClick={() => {
                const p = profiles.data?.find((p) => p.id === b.profile);
                if (p) {
                  connect(p);
                  remote.go(b.prefix);
                  local.go(b.local);
                }
              }}
            >
              <Bookmark size={14} />
              <span>{b.label}</span>
            </button>
            <button
              aria-label={`Remove bookmark ${b.label}`}
              onClick={() =>
                preferences.set({
                  bookmarks: preferences.bookmarks.filter((_, n) => n !== i),
                })
              }
            >
              <X size={12} />
            </button>
          </div>
        ))}
        <div className="sidebar-bottom">
          <div className="keychain-note">
            <ShieldCheck size={16} />
            <div>
              Keys stay with you.<span>Secured by your OS keychain</span>
            </div>
          </div>
          <button className="sidebar-link" onClick={() => setSettings(true)}>
            <Settings2 size={16} /> Settings
            <SlidersHorizontal size={13} className="muted" />
          </button>
          <div className="vibe-note">
            <span className="tiny-dot" /> 100% vibecoded. Just for fun.
          </div>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <div className="topbar-title">
            <span className="section-index">01 /</span>
            <h1>File explorer</h1>
          </div>
          <div className="topbar-right">
            <span className="environment">
              <span
                className={`tiny-dot ${profile && remoteFiles.isSuccess ? "online" : ""}`}
              />
              {profile
                ? remoteFiles.isSuccess
                  ? "Bucket connected"
                  : remoteFiles.isError
                    ? "Connection error"
                    : "Connecting…"
                : "No active connection"}
            </span>
            <span className="divider" />
            <Button
              size="icon"
              variant="ghost"
              aria-label="Settings"
              onClick={() => setSettings(true)}
            >
              <Settings2 size={17} />
            </Button>
          </div>
        </header>
        {!desktop && (
          <div className="browser-notice">
            <Terminal size={15} />
            <span>
              Desktop preview · Launch with <code>npm run desktop dev</code> for
              filesystem and S3 access.
            </span>
          </div>
        )}
        <div className="workspace-toolbar">
          <div className="connection-selector">
            <Cloud size={17} />
            <select
              aria-label="Active connection"
              value={profile?.id ?? ""}
              onChange={(e) => {
                const p = profiles.data?.find((p) => p.id === e.target.value);
                if (p) connect(p);
              }}
            >
              <option value="">Select a connection</option>
              {profiles.data?.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name} / {p.bucket}
                </option>
              ))}
            </select>
            <ChevronDown size={13} />
          </div>
          <div className="toolbar-actions">
            <Button
              disabled={!profile || !local.path}
              onClick={() => setSync(true)}
            >
              <Workflow size={15} /> Synchronize
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Refresh listings"
              disabled={!desktop}
              onClick={() => void run(refresh)}
            >
              <RefreshCw size={15} />
            </Button>
            <div className="view-toggle" aria-label="Pane layout">
              {(["dual", "local", "remote"] as const).map((view) => (
                <button
                  key={view}
                  className={preferences.view === view ? "active" : ""}
                  aria-label={`${view} pane layout`}
                  title={`${view} pane layout`}
                  onClick={() => preferences.set({ view })}
                >
                  {view === "dual" ? (
                    <LayoutPanelLeft size={15} />
                  ) : view === "local" ? (
                    <HardDrive size={15} />
                  ) : (
                    <Cloud size={15} />
                  )}
                </button>
              ))}
            </div>
          </div>
        </div>
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
        <div className={`panes ${preferences.view}`}>
          {preferences.view !== "remote" && (
            <section
              className="pane local-pane"
              onDragOver={(e) => {
                if (
                  e.dataTransfer.types.includes("application/x-voids3-remote")
                )
                  e.preventDefault();
              }}
              onDrop={(e) => {
                const value = e.dataTransfer.getData(
                  "application/x-voids3-remote",
                );
                if (value) {
                  e.preventDefault();
                  void run(() => download(JSON.parse(value) as string[]));
                }
              }}
            >
              <div className="pane-heading">
                <span>
                  <HardDrive size={16} /> LOCAL FILES
                </span>
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
                error={localFiles.error ? message(localFiles.error) : undefined}
                connected={desktop}
                onAction={(name, entry) => action(false, name, entry)}
                onRetry={() => void localFiles.refetch()}
              />
              <div className="pane-action">
                <span className="muted">
                  {localSelection.length
                    ? `${localSelection.length} item(s) ready`
                    : "Select files or drop them into remote"}
                </span>
                <Button
                  disabled={!profile || readonly || !localSelection.length}
                  onClick={() => void run(() => upload(localSelection))}
                >
                  <ArrowUpFromLine size={14} /> Upload selection
                </Button>
              </div>
            </section>
          )}
          {preferences.view !== "local" && (
            <section
              className="pane remote-pane"
              onDragOver={(e) => {
                if (e.dataTransfer.types.includes("application/x-voids3-local"))
                  e.preventDefault();
              }}
              onDrop={(e) => {
                const value = e.dataTransfer.getData(
                  "application/x-voids3-local",
                );
                if (value && !readonly) {
                  e.preventDefault();
                  void run(() => upload(JSON.parse(value) as string[]));
                }
              }}
            >
              <div className="pane-heading">
                <span>
                  <Cloud size={16} /> REMOTE STORAGE
                  {profile && (
                    <span className="provider-tag">
                      {profile.provider.toUpperCase()}
                    </span>
                  )}
                </span>
                <div className="split-button">
                  <Button
                    size="small"
                    variant="default"
                    disabled={!desktop || readonly}
                    onClick={() => void run(() => upload())}
                  >
                    <Plus size={14} /> Upload files
                  </Button>
                  <Dropdown.Root>
                    <Dropdown.Trigger asChild>
                      <Button
                        size="small"
                        variant="default"
                        disabled={!profile || readonly}
                        aria-label="More upload options"
                      >
                        <ChevronDown size={13} />
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
              <div className="pane-action">
                <span className="muted">
                  {readonly
                    ? "Read-only connection"
                    : "Existing objects are never overwritten"}
                </span>
                <Button
                  disabled={!profile || !remoteSelection.length}
                  onClick={() => void run(() => download())}
                >
                  <ArrowDownToLine size={14} /> Download
                </Button>
              </div>
            </section>
          )}
        </div>
        <Queue
          jobs={jobs}
          open={preferences.queueOpen}
          toggle={() => preferences.set({ queueOpen: !preferences.queueOpen })}
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
        <footer className="statusbar">
          <span>
            {profile ? (
              <>
                <span className="tiny-dot online" />
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
      </main>
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
              if (!prompt.content) onClose();
              else if (prompt.label === "Done") onClose();
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
