import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "../App";
import { ConnectionDialog } from "../components/ConnectionDialog";
import { Queue } from "../components/Queue";
import { usePreferences } from "../store";
import { api, bytes, parent, type Transfer } from "../lib/api";
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: false,
  api: vi.fn(),
}));
const mockApi = vi.mocked(api);
function wrap(
  ui: React.ReactNode,
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } }),
) {
  return render(
    <QueryClientProvider client={client}>{ui}</QueryClientProvider>,
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  usePreferences.setState({
    theme: "dark",
    view: "dual",
    queueOpen: true,
    hidden: false,
    bookmarks: [],
  });
});
describe("desktop shell", () => {
  it("shows honest browser mode without fabricated files or IPC access", () => {
    wrap(<App />);
    expect(screen.getByText(/Desktop preview/)).toBeInTheDocument();
    expect(screen.getByText("No connections yet")).toBeInTheDocument();
    expect(screen.getByText("No transfers queued")).toBeInTheDocument();
    expect(mockApi).not.toHaveBeenCalled();
  });
  it("changes layouts and opens an accessible connection dialog", async () => {
    wrap(<App />);
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "remote pane layout" }),
    );
    expect(
      screen.getByRole("button", { name: "remote pane layout" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.queryByRole("listbox", { name: "Local files" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "New connection" }));
    expect(
      screen.getByRole("dialog", { name: "New connection" }),
    ).toBeInTheDocument();
  });
  it("discloses navigation and the transfer queue with keyboard-accessible state", async () => {
    wrap(<App />);
    const user = userEvent.setup();
    const navigation = screen.getByRole("button", {
      name: "Toggle navigation",
    });
    await user.click(navigation);
    expect(navigation).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(navigation).toHaveAttribute("aria-expanded", "false");
    expect(navigation).toHaveFocus();
    const queue = screen.getByRole("button", { name: /Transfer queue/ });
    await user.click(queue);
    expect(queue).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("No transfers queued")).not.toBeInTheDocument();
    await user.click(queue);
    expect(queue).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("No transfers queued")).toBeInTheDocument();
  });
  it("opens real transfer, bookmark, and activity views from the Figma navigation", async () => {
    wrap(<App />);
    const user = userEvent.setup();
    const nav = within(
      screen.getByRole("navigation", { name: "Main navigation" }),
    );
    await user.click(nav.getByRole("button", { name: "Transfers" }));
    expect(
      screen.queryByRole("listbox", { name: "Local files" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("combobox", { name: "Filter transfers" }),
    ).toBeInTheDocument();
    await user.click(nav.getByRole("button", { name: "Files" }));
    expect(
      screen.getByRole("listbox", { name: "Local files" }),
    ).toBeInTheDocument();
    await user.click(nav.getByRole("button", { name: "Bookmarks" }));
    expect(screen.getByRole("dialog", { name: "Bookmarks" })).toHaveTextContent(
      "No bookmarks yet",
    );
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(screen.getByRole("dialog", { name: "Activity" })).toHaveTextContent(
      "No current alerts",
    );
  });
  it("focuses search and restores focus when its filter is dismissed", async () => {
    wrap(<App />);
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Search loaded files" }),
    );
    const search = screen.getByRole("textbox", { name: "Filter loaded files" });
    expect(search).toHaveFocus();
    await user.type(search, "archive");
    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("textbox", { name: "Filter loaded files" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Search loaded files" }),
    ).toHaveFocus();
  });
  it("opens a saved pair with one remote navigation step", async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    client.setQueryData(
      ["profiles"],
      [
        {
          id: "test-profile",
          name: "Test connection",
          provider: "r2",
          bucket: "assets",
          prefix: "",
          region: "auto",
          endpoint: "https://example.com",
          read_only: false,
          path_style: true,
          allow_http: false,
        },
      ],
    );
    client.setQueryData(["home"], "C:/base");
    usePreferences.setState({
      bookmarks: [
        {
          label: "assets/releases",
          profile: "test-profile",
          prefix: "releases/",
          local: "C:/exports",
        },
      ],
    });
    wrap(<App />, client);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Bookmarks" }));
    await user.click(screen.getByRole("button", { name: /^assets\/releases/ }));
    expect(
      screen.queryByRole("dialog", { name: "Bookmarks" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Edit Remote prefix" }),
    ).toHaveTextContent("releases/");
    expect(
      screen.getByRole("button", { name: "Edit Local directory" }),
    ).toHaveTextContent("C:/exports");
  });
  it("resizes the split panes with the keyboard", () => {
    wrap(<App />);
    const handle = screen.getByRole("separator", { name: "Resize file panes" });
    Object.defineProperty(handle.parentElement, "clientWidth", { value: 1000 });
    fireEvent.keyDown(handle, { key: "ArrowRight" });
    expect(handle).toHaveAttribute("aria-valuenow", "55");
    fireEvent.keyDown(handle, { key: "Home" });
    expect(handle).toHaveAttribute("aria-valuenow", "30");
  });
});
describe("connection form", () => {
  it("builds an EU R2 endpoint and submits secrets only to save IPC", async () => {
    mockApi.mockResolvedValue(null);
    const onSaved = vi.fn();
    wrap(<ConnectionDialog onClose={vi.fn()} onSaved={onSaved} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Connection name"), "R2 test");
    await user.type(screen.getByLabelText(/R2 account ID/), "test-account");
    await user.selectOptions(screen.getByLabelText("Jurisdiction"), "eu");
    expect(screen.getByLabelText("Endpoint URL")).toHaveValue(
      "https://test-account.eu.r2.cloudflarestorage.com",
    );
    await user.type(screen.getByLabelText("Bucket"), "assets");
    await user.type(screen.getByLabelText("Access key ID"), "test-access");
    await user.type(screen.getByLabelText("Secret access key"), "test-secret");
    await user.click(screen.getByRole("button", { name: "Save connection" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith(
      "save_profile",
      expect.objectContaining({
        profile: expect.objectContaining({ region: "auto", bucket: "assets" }),
        credentials: expect.objectContaining({ secret_key: "test-secret" }),
      }),
    );
    expect(localStorage.getItem("voids3-ui-v1") ?? "").not.toContain(
      "test-secret",
    );
  });
  it("can finish saving a newly tested connection without re-entering credentials", async () => {
    mockApi.mockResolvedValue(null);
    const onSaved = vi.fn();
    wrap(<ConnectionDialog onClose={vi.fn()} onSaved={onSaved} />);
    const user = userEvent.setup();
    await user.type(screen.getByLabelText("Connection name"), "Saved test");
    await user.type(
      screen.getByLabelText("Endpoint URL"),
      "https://example.com",
    );
    await user.type(screen.getByLabelText("Bucket"), "assets");
    await user.type(screen.getByLabelText("Access key ID"), "test-access");
    await user.type(screen.getByLabelText("Secret access key"), "test-secret");
    await user.click(screen.getByRole("button", { name: "Save & test" }));
    expect(
      await screen.findByText("Bucket access verified."),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Secret access key")).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "Save connection" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledOnce());
    expect(mockApi).toHaveBeenLastCalledWith(
      "save_profile",
      expect.objectContaining({ credentials: null }),
    );
  });
  it("surfaces actionable backend errors", async () => {
    mockApi.mockRejectedValue("Credential store: locked");
    wrap(
      <ConnectionDialog
        initial={{
          id: crypto.randomUUID(),
          name: "Test",
          provider: "r2",
          endpoint: "https://example.com",
          region: "auto",
          bucket: "test",
          prefix: "",
          path_style: true,
          read_only: false,
          allow_http: false,
        }}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />,
    );
    fireEvent.submit(
      screen.getByRole("button", { name: "Save connection" }).closest("form")!,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Credential store: locked",
    );
  });
});
describe("queue", () => {
  it("displays actual progress and exposes resume/cancel for failures", async () => {
    const job = {
      id: "one",
      key: "file.zip",
      direction: "upload",
      local_path: "C:/file.zip",
      status: "failed",
      error: "Access denied",
      size: 100,
      bytes_done: 25,
    } as Transfer;
    const control = vi.fn();
    wrap(
      <Queue
        jobs={[job]}
        open
        toggle={vi.fn()}
        control={control}
        clear={vi.fn()}
      />,
    );
    expect(screen.getByText("25%")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Access denied");
    await userEvent.click(
      screen.getByRole("button", { name: "Resume file.zip" }),
    );
    expect(control).toHaveBeenCalledWith("one", "resume");
  });
});
describe("path and size utilities", () => {
  it("preserves local roots and remote prefix semantics", () => {
    expect(parent("C:\\Users\\test")).toBe("C:\\Users");
    expect(parent("C:\\Users")).toBe("C:\\");
    expect(parent("a/b/", true)).toBe("a/");
    expect(parent("a/", true)).toBe("");
  });
  it("formats real byte counts", () => {
    expect(bytes(1024)).toBe("1.0 KiB");
    expect(bytes(0)).toBe("0 B");
    expect(bytes(Number.NaN)).toBe("-");
  });
});
