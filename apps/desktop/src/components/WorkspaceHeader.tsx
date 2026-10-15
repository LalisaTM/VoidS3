import { useEffect, useRef, useState } from "react";
import {
  Bell,
  Cloud,
  HardDrive,
  LayoutPanelLeft,
  Menu,
  Search,
  X,
} from "lucide-react";
import { Button } from "./ui/button";

export function WorkspaceHeader({
  view,
  onView,
  onBookmarks,
  onActivity,
  attention,
  search,
  onSearch,
  navigationOpen,
  onNavigation,
  layout,
  onLayout,
}: {
  view: "files" | "transfers";
  onView: (view: "files" | "transfers") => void;
  onBookmarks: () => void;
  onActivity: () => void;
  attention: boolean;
  search: string;
  onSearch: (value: string) => void;
  navigationOpen: boolean;
  onNavigation: () => void;
  layout: "dual" | "local" | "remote";
  onLayout: (layout: "dual" | "local" | "remote") => void;
}) {
  const [searchOpen, setSearchOpen] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const searchButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (searchOpen) input.current?.focus();
  }, [searchOpen]);
  return (
    <header className="app-topbar">
      <Button
        id="navigation-toggle"
        className="navigation-toggle"
        variant="ghost"
        size="icon"
        aria-label="Toggle navigation"
        aria-controls="workspace-navigation"
        aria-expanded={navigationOpen}
        onClick={onNavigation}
      >
        {navigationOpen ? <X size={18} /> : <Menu size={18} />}
      </Button>
      <div className="brand">
        <span className="brand-glyph" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        <span>
          Void<strong>S3</strong>
        </span>
      </div>
      <nav className="main-nav" aria-label="Main navigation">
        <button
          aria-current={view === "files" ? "page" : undefined}
          onClick={() => onView("files")}
        >
          Files
        </button>
        <button
          aria-current={view === "transfers" ? "page" : undefined}
          onClick={() => onView("transfers")}
        >
          Transfers
        </button>
        <button onClick={onBookmarks}>Bookmarks</button>
      </nav>
      <div className="top-actions">
        <div className="view-toggle" aria-label="Pane layout">
          {(["dual", "local", "remote"] as const).map((value) => (
            <button
              key={value}
              aria-label={`${value} pane layout`}
              title={`${value} pane layout`}
              aria-pressed={layout === value}
              className={layout === value ? "active" : ""}
              onClick={() => {
                onLayout(value);
                onView("files");
              }}
            >
              {value === "dual" ? (
                <LayoutPanelLeft size={15} />
              ) : value === "local" ? (
                <HardDrive size={15} />
              ) : (
                <Cloud size={15} />
              )}
            </button>
          ))}
        </div>
        {searchOpen && (
          <input
            ref={input}
            className="search-field"
            aria-label="Filter loaded files"
            title="Filters only the files already loaded in both panes"
            placeholder="Filter loaded files…"
            value={search}
            onChange={(e) => onSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.stopPropagation();
                setSearchOpen(false);
                onSearch("");
                searchButton.current?.focus();
              }
            }}
          />
        )}
        <Button
          ref={searchButton}
          variant="ghost"
          size="icon"
          aria-label="Search loaded files"
          aria-expanded={searchOpen}
          onClick={() => {
            setSearchOpen(!searchOpen);
            onView("files");
            if (searchOpen) onSearch("");
          }}
        >
          <Search size={17} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="activity-button"
          aria-label="Activity"
          onClick={onActivity}
        >
          <Bell size={17} />
          {attention && <span className="attention-dot" />}
        </Button>
      </div>
    </header>
  );
}
