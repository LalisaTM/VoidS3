import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { api, type FileEntry, type Profile, type Transfer } from "../lib/api";
import { usePreferences } from "../store";

vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: true,
  api: vi.fn(),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: async () => vi.fn() }),
}));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 38,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        start: index * 38,
        size: 38,
      })),
    scrollToIndex: vi.fn(),
  }),
}));
const mockApi = vi.mocked(api);
const profile: Profile = {
  id: "test-profile",
  name: "Test storage",
  provider: "r2",
  bucket: "assets",
  prefix: "",
  region: "auto",
  endpoint: "https://example.invalid",
  read_only: false,
  path_style: true,
  allow_http: false,
};
const files: FileEntry[] = ["alpha.txt", "beta.txt"].map((name) => ({
  name,
  path: name,
  is_dir: false,
  size: 20,
  modified: null,
  etag: `etag-${name}`,
}));
const jobs: Transfer[] = ["uploading", "paused", "completed"].map(
  (status, index) => ({
    id: `job-${index}`,
    profile_id: `connection-${index}`,
    key: `file-${index}`,
    local_path: `C:/test/file-${index}`,
    direction: "upload",
    size: 20,
    bytes_done: 10,
    status,
    error: null,
    created: 0,
    upload_id: null,
  }),
);
function mount(readOnly = false) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  const saved = { ...profile, read_only: readOnly };
  client.setQueryData(["profiles"], [saved]);
  client.setQueryData(["home"], "C:/test");
  client.setQueryData(
    ["local", "C:/test", false],
    files.map((file) => ({ ...file, path: `C:/test/${file.name}` })),
  );
  client.setQueryData(["remote", profile.id, ""], {
    pages: [{ entries: files, next_token: null }],
    pageParams: [null],
  });
  client.setQueryData(["transfers"], jobs);
  mockApi.mockImplementation(async (action) => {
    if (action === "profiles") return [saved];
    if (action === "home") return "C:/test";
    if (action === "local_list")
      return files.map((file) => ({ ...file, path: `C:/test/${file.name}` }));
    if (action === "remote_list") return { entries: files, next_token: null };
    if (action === "transfers") return jobs;
    if (action === "transfer_control_all")
      return { requested: 2, failed: 0, errors: [] };
    if (action === "delete_entries")
      return {
        deleted: [files[0].path],
        failed: 1,
        errors: ["beta.txt: object changed"],
      };
    throw new Error(`Unexpected test action: ${action}`);
  });
  return render(
    <QueryClientProvider client={client}>
      <App />
    </QueryClientProvider>,
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
describe("workspace actions", () => {
  it("sends selected local files through conflict preflight before uploading", async () => {
    mount();
    const user = userEvent.setup();
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Active connection" }),
      profile.id,
    );
    const local = screen.getByRole("listbox", { name: "Local files" });
    fireEvent.click(within(local).getAllByRole("option")[0]);
    const fallback = mockApi.getMockImplementation()!;
    mockApi.mockImplementation(async (action, args) => {
      if (action === "prepare_upload")
        return {
          id: "upload-plan",
          total: 1,
          conflicts: [
            {
              index: 0,
              path: "C:/test/alpha.txt",
              key: "alpha.txt",
              source: { size: 20, modified: null },
              destination: { size: 40, modified: null },
              can_replace: true,
            },
          ],
        };
      if (action === "commit_upload")
        return { queued: ["new-upload"], skipped: 0, failed: 0, errors: [] };
      return fallback(action, args);
    });
    await user.click(
      screen.getByRole("button", { name: /Upload.*Local → S3/ }),
    );
    expect(mockApi).toHaveBeenCalledWith("prepare_upload", {
      id: profile.id,
      paths: ["C:/test/alpha.txt"],
      prefix: "",
    });
    expect(
      screen.getByRole("dialog", { name: "File already exists" }),
    ).toHaveTextContent("alpha.txt");
    expect(mockApi).not.toHaveBeenCalledWith("upload", expect.anything());
    await user.click(screen.getByRole("button", { name: "Replace" }));
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("commit_upload", {
        id: "upload-plan",
        choices: [{ index: 0, action: "replace" }],
      }),
    );
  });
  it("pauses all connections and confirms cancellation independently of the queue filter", async () => {
    mount();
    const user = userEvent.setup();
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Filter transfers" }),
      "completed",
    );
    await user.click(screen.getByRole("button", { name: "Pause all" }));
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("transfer_control_all", {
        operation: "pause",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel all" }));
    const dialog = screen.getByRole("dialog", {
      name: "Cancel all transfers?",
    });
    expect(dialog).toHaveTextContent("across every connection");
    expect(mockApi).not.toHaveBeenCalledWith("transfer_control_all", {
      operation: "cancel",
    });
    await user.click(
      within(dialog).getByRole("button", { name: "Cancel all transfers" }),
    );
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("transfer_control_all", {
        operation: "cancel",
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Resume all" }));
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("transfer_control_all", {
        operation: "resume",
      }),
    );
  });
  it("deletes a selected group with listed ETags and keeps failed items selected for retry", async () => {
    mount();
    const user = userEvent.setup();
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Active connection" }),
      profile.id,
    );
    const list = screen.getByRole("listbox", { name: "Remote files" });
    const rows = within(list).getAllByRole("option");
    fireEvent.click(rows[0]);
    fireEvent.click(rows[1], { ctrlKey: true });
    fireEvent.focus(list);
    await user.click(
      screen.getByRole("button", { name: "Delete selected items" }),
    );
    const dialog = screen.getByRole("dialog", { name: "Delete 2 file(s)?" });
    expect(dialog).toHaveTextContent("alpha.txt");
    expect(dialog).toHaveTextContent("beta.txt");
    expect(mockApi).not.toHaveBeenCalledWith(
      "delete_entries",
      expect.anything(),
    );
    await user.click(
      within(dialog).getByRole("button", { name: "Delete files" }),
    );
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("delete_entries", {
        id: profile.id,
        entries: files.map(({ path, etag }) => ({ path, etag })),
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(
      within(list).getAllByRole("option", { selected: true }),
    ).toHaveLength(1);
    expect(
      within(list).getByRole("option", { selected: true }),
    ).toHaveTextContent("beta.txt");
    expect(screen.getByRole("alert")).toHaveTextContent("object changed");
    fireEvent.keyDown(list, { key: "Delete" });
    expect(
      screen.getByRole("dialog", { name: "Delete 1 file(s)?" }),
    ).toHaveTextContent("beta.txt");
  });
  it("recycles local selections and blocks remote deletion on a read-only connection", async () => {
    mount(true);
    const user = userEvent.setup();
    const local = screen.getByRole("listbox", { name: "Local files" });
    fireEvent.click(within(local).getAllByRole("option")[0]);
    fireEvent.keyDown(local, { key: "Delete" });
    expect(
      screen.getByRole("dialog", { name: "Recycle 1 item(s)?" }),
    ).toHaveTextContent("operating system recycle bin");
    await user.keyboard("{Escape}");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Active connection" }),
      profile.id,
    );
    const remote = screen.getByRole("listbox", { name: "Remote files" });
    fireEvent.click(within(remote).getAllByRole("option")[0]);
    fireEvent.focus(remote);
    expect(
      screen.getByRole("button", { name: "Delete selected items" }),
    ).toBeDisabled();
    fireEvent.keyDown(remote, { key: "Delete" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
