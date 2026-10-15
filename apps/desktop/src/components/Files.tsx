import { useMemo, useRef, useState, type MouseEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import * as Dropdown from "@radix-ui/react-dropdown-menu";
import {
  ArrowDown,
  ArrowUp,
  ChevronRight,
  Clipboard,
  File,
  FileCode2,
  Folder,
  FolderPlus,
  MoreHorizontal,
  Search,
  UploadCloud,
  DownloadCloud,
} from "lucide-react";
import { bytes, type FileEntry } from "../lib/api";
import { Button } from "./ui/button";
export function Files({
  entries,
  selected,
  onSelect,
  onOpen,
  loading,
  error,
  remote,
  connected,
  writable = true,
  onAction,
  onRetry,
}: {
  entries: FileEntry[];
  selected: string[];
  onSelect: (paths: string[]) => void;
  onOpen: (entry: FileEntry) => void;
  loading: boolean;
  error?: string;
  remote?: boolean;
  connected: boolean;
  writable?: boolean;
  onAction: (action: string, entry?: FileEntry) => void;
  onRetry: () => void;
}) {
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<"name" | "size" | "modified">("name");
  const [descending, setDescending] = useState(false);
  const [menu, setMenu] = useState<string | null>(null);
  const anchor = useRef(0);
  const container = useRef<HTMLDivElement>(null);
  const filtered = useMemo(
    () =>
      entries
        .filter((e) =>
          e.name.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
        )
        .sort((a, b) => {
          if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
          const cmp =
            sort === "name"
              ? a.name.localeCompare(b.name)
              : (a[sort] ?? 0) - (b[sort] ?? 0);
          return descending ? -cmp : cmp;
        }),
    [entries, search, sort, descending],
  );
  const virtual = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => container.current,
    estimateSize: () => 38,
    overscan: 15,
  });
  function select(event: MouseEvent, index: number) {
    const entry = filtered[index];
    if (event.shiftKey)
      onSelect(
        filtered
          .slice(
            Math.min(anchor.current, index),
            Math.max(anchor.current, index) + 1,
          )
          .map((e) => e.path),
      );
    else if (event.ctrlKey || event.metaKey) {
      onSelect(
        selected.includes(entry.path)
          ? selected.filter((p) => p !== entry.path)
          : [...selected, entry.path],
      );
      anchor.current = index;
    } else {
      onSelect([entry.path]);
      anchor.current = index;
    }
  }
  function sortBy(value: typeof sort) {
    if (sort === value) setDescending((d) => !d);
    else {
      setSort(value);
      setDescending(false);
    }
  }
  const SortIcon = descending ? ArrowDown : ArrowUp;
  return (
    <div className="file-area">
      <div className="filter-bar">
        <Search size={14} />
        <input
          aria-label={remote ? "Filter remote files" : "Filter local files"}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Filter loaded files…"
        />
        <span className="mono">{filtered.length}</span>
        <Button
          aria-label={remote ? "New remote folder" : "New local folder"}
          size="icon"
          variant="ghost"
          disabled={!connected || !writable}
          onClick={() => onAction("mkdir")}
        >
          <FolderPlus size={15} />
        </Button>
      </div>
      <div className="file-columns">
        <button onClick={() => sortBy("name")}>
          Name {sort === "name" && <SortIcon size={11} />}
        </button>
        <button onClick={() => sortBy("size")}>
          Size {sort === "size" && <SortIcon size={11} />}
        </button>
        <button onClick={() => sortBy("modified")}>
          Modified {sort === "modified" && <SortIcon size={11} />}
        </button>
        <span />
      </div>
      <div
        ref={container}
        className="file-scroll"
        role="listbox"
        aria-label={remote ? "Remote files" : "Local files"}
        aria-multiselectable="true"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.target !== e.currentTarget) return;
          if (
            filtered.length &&
            ["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)
          ) {
            e.preventDefault();
            const current = filtered.findIndex(
              (entry) => entry.path === selected.at(-1),
            );
            const index =
              e.key === "Home"
                ? 0
                : e.key === "End"
                  ? filtered.length - 1
                  : Math.max(
                      0,
                      Math.min(
                        filtered.length - 1,
                        current + (e.key === "ArrowDown" ? 1 : -1),
                      ),
                    );
            if (e.shiftKey)
              onSelect(
                filtered
                  .slice(
                    Math.min(anchor.current, index),
                    Math.max(anchor.current, index) + 1,
                  )
                  .map((entry) => entry.path),
              );
            else {
              onSelect([filtered[index].path]);
              anchor.current = index;
            }
            virtual.scrollToIndex(index);
          }
          if ((e.ctrlKey || e.metaKey) && e.key === "a") {
            e.preventDefault();
            onSelect(filtered.map((f) => f.path));
          }
          if (e.key === "Escape") onSelect([]);
          if (e.key === "Enter" && selected.length === 1) {
            const entry = filtered.find((f) => f.path === selected[0]);
            if (entry) onOpen(entry);
          }
        }}
      >
        {error ? (
          <div className="empty-state">
            <div className="error-box" role="alert">
              {error}
            </div>
            <Button onClick={onRetry}>Try again</Button>
          </div>
        ) : loading ? (
          <div className="empty-state">
            <span className="loading-dot" />
            Reading directory…
          </div>
        ) : !filtered.length ? (
          <div className="empty-state">
            <div className="empty-icon">
              {remote ? <UploadCloud size={27} /> : <Folder size={27} />}
            </div>
            <h3>
              {!connected
                ? remote
                  ? "Your storage, connected."
                  : "Your local workspace"
                : search
                  ? "No matching files"
                  : "This folder is empty"}
            </h3>
            <p>
              {!connected
                ? remote
                  ? "Add an S3 connection to browse your buckets and move files."
                  : "Your files appear here in the desktop app."
                : search
                  ? "This filter searches the current loaded listing."
                  : "Create a folder or transfer files to get started."}
            </p>
            {remote && !connected && (
              <Button onClick={() => onAction("connect")}>
                <span>+</span> Add connection
              </Button>
            )}
          </div>
        ) : (
          <div
            style={{
              height: virtual.getTotalSize(),
              position: "relative",
              width: "100%",
            }}
          >
            {virtual.getVirtualItems().map((row) => {
              const entry = filtered[row.index];
              return (
                <div
                  key={entry.path}
                  role="option"
                  aria-selected={selected.includes(entry.path)}
                  className={`file-row ${selected.includes(entry.path) ? "selected" : ""}`}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: row.size,
                    transform: `translateY(${row.start}px)`,
                  }}
                  onClick={(e) => select(e, row.index)}
                  onDoubleClick={() => onOpen(entry)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    onSelect([entry.path]);
                    setMenu(entry.path);
                  }}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData(
                      remote
                        ? "application/x-voids3-remote"
                        : "application/x-voids3-local",
                      JSON.stringify(
                        selected.includes(entry.path) ? selected : [entry.path],
                      ),
                    );
                  }}
                >
                  <span className="filename">
                    {entry.is_dir ? (
                      <Folder size={16} className="folder-icon" />
                    ) : /\.(json|tsx?|rs|ya?ml|md|css|html)$/.test(
                        entry.name,
                      ) ? (
                      <FileCode2 size={16} />
                    ) : (
                      <File size={16} />
                    )}
                    <span>{entry.name}</span>
                    {entry.is_dir && (
                      <ChevronRight size={12} className="folder-chevron" />
                    )}
                  </span>
                  <span className="mono muted">
                    {entry.is_dir ? "—" : bytes(entry.size)}
                  </span>
                  <span className="muted date">
                    {entry.modified
                      ? new Date(entry.modified * 1000).toLocaleDateString(
                          undefined,
                          { month: "short", day: "numeric", year: "2-digit" },
                        )
                      : "—"}
                  </span>
                  <Dropdown.Root
                    open={menu === entry.path}
                    onOpenChange={(open) => setMenu(open ? entry.path : null)}
                  >
                    <Dropdown.Trigger asChild>
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Actions for ${entry.name}`}
                        onClick={(e) => e.stopPropagation()}
                      >
                        <MoreHorizontal size={15} />
                      </Button>
                    </Dropdown.Trigger>
                    <Dropdown.Portal>
                      <Dropdown.Content className="dropdown" align="end">
                        {!entry.is_dir && (
                          <>
                            <Dropdown.Item
                              onSelect={() => onAction("transfer", entry)}
                            >
                              {remote ? (
                                <DownloadCloud size={14} />
                              ) : (
                                <UploadCloud size={14} />
                              )}{" "}
                              {remote ? "Download" : "Upload"}
                            </Dropdown.Item>
                            <Dropdown.Item
                              onSelect={() => onAction("properties", entry)}
                            >
                              Properties / text preview
                            </Dropdown.Item>
                          </>
                        )}
                        <Dropdown.Item
                          onSelect={() => onAction("copy-path", entry)}
                        >
                          <Clipboard size={14} /> Copy{" "}
                          {remote ? "object key" : "path"}
                        </Dropdown.Item>
                        {!entry.is_dir && remote && (
                          <>
                            <Dropdown.Item
                              onSelect={() => onAction("presign", entry)}
                            >
                              Copy signed download URL
                            </Dropdown.Item>
                            <Dropdown.Item
                              disabled={!writable}
                              onSelect={() => onAction("copy", entry)}
                            >
                              Copy object…
                            </Dropdown.Item>
                          </>
                        )}
                        {!remote && (
                          <Dropdown.Item
                            onSelect={() => onAction("rename", entry)}
                          >
                            Rename…
                          </Dropdown.Item>
                        )}
                        {(!entry.is_dir || !remote) && (
                          <Dropdown.Item
                            className="destructive"
                            disabled={!writable}
                            onSelect={() => onAction("delete", entry)}
                          >
                            {remote ? "Delete object…" : "Move to recycle bin…"}
                          </Dropdown.Item>
                        )}
                      </Dropdown.Content>
                    </Dropdown.Portal>
                  </Dropdown.Root>
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="pane-footer">
        <span>
          {selected.length
            ? `${selected.length} selected`
            : `${filtered.length} items`}
        </span>
        <span>
          {bytes(filtered.reduce((sum, e) => sum + (e.is_dir ? 0 : e.size), 0))}
        </span>
      </div>
    </div>
  );
}
