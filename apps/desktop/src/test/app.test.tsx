import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "../App";
import { ConnectionDialog } from "../components/ConnectionDialog";
import { Queue } from "../components/Queue";
import { api, bytes, parent, type Transfer } from "../lib/api";
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: false,
  api: vi.fn(),
}));
const mockApi = vi.mocked(api);
function wrap(ui: React.ReactNode) {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      {ui}
    </QueryClientProvider>,
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
});
describe("desktop shell", () => {
  it("shows honest browser mode without fabricated files or IPC access", () => {
    wrap(<App />);
    expect(screen.getByText(/Desktop preview/)).toBeInTheDocument();
    expect(screen.getByText("No connections yet")).toBeInTheDocument();
    expect(
      screen.getByText("All clear. Ready when you are."),
    ).toBeInTheDocument();
    expect(mockApi).not.toHaveBeenCalled();
  });
  it("changes layouts and opens an accessible connection dialog", async () => {
    wrap(<App />);
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "remote pane layout" }),
    );
    expect(
      screen.queryByRole("listbox", { name: "Local files" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "New connection" }));
    expect(
      screen.getByRole("dialog", { name: "New connection" }),
    ).toBeInTheDocument();
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
    expect(bytes(Number.NaN)).toBe("—");
  });
});
