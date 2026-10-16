import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useUpdater, type UpdateProgress } from "../lib/useUpdater";
import { UpdatePanel } from "../components/UpdatePanel";
import { SettingsDialog } from "../components/SettingsDialog";

const native = vi.hoisted(() => ({
  progress: undefined as undefined | ((event: UpdateProgress) => void),
}));
vi.mock("@tauri-apps/api/core", async (original) => ({
  ...(await original<typeof import("@tauri-apps/api/core")>()),
  invoke: vi.fn(),
  Channel: class {
    set onmessage(handler: (event: UpdateProgress) => void) {
      native.progress = handler;
    }
  },
}));
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: true,
}));
const mockedInvoke = vi.mocked(invoke);
const close = vi.fn();
const available = {
  supported: true,
  update: { version: "0.3.0", notes: "Changes supplied by GitHub Releases." },
};
function Harness({
  active = 0,
  automatic = false,
  settings = false,
}: {
  active?: number;
  automatic?: boolean;
  settings?: boolean;
}) {
  const updater = useUpdater(automatic);
  return settings ? (
    <SettingsDialog
      updater={updater}
      activeTransfers={active}
      onClose={close}
    />
  ) : (
    <UpdatePanel updater={updater} activeTransfers={active} />
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  native.progress = undefined;
});
afterEach(() => vi.useRealTimers());
describe("GitHub release updater", () => {
  it("requires confirmation and displays real download/signature stages before reporting a retryable failure", async () => {
    let rejectInstall: (error: Error) => void = () => {};
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "check_for_updates") return available;
      if (command === "install_update")
        return new Promise((_, reject) => {
          rejectInstall = reject;
        });
      throw new Error(`Unexpected command: ${command}`);
    });
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(screen.getByText("Version 0.3.0 is available")).toBeInTheDocument();
    await user.click(screen.getByText("Release notes"));
    expect(screen.getByText(available.update.notes)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Install update…" }));
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      "install_update",
      expect.anything(),
    );
    await user.click(
      screen.getByRole("button", { name: "Install and restart" }),
    );
    expect(mockedInvoke).toHaveBeenCalledWith("install_update", {
      version: "0.3.0",
      onEvent: expect.anything(),
    });
    act(() =>
      native.progress?.({ stage: "downloading", downloaded: 512, total: 1024 }),
    );
    expect(screen.getByRole("progressbar")).toHaveAttribute("value", "50");
    act(() => native.progress?.({ stage: "verifying" }));
    expect(screen.getByRole("status")).toHaveTextContent(
      "Verifying update signature",
    );
    await act(async () =>
      rejectInstall(new Error("Signature verification failed")),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Signature verification failed",
    );
    expect(
      screen.getByRole("button", { name: "Install update…" }),
    ).toBeEnabled();
  });
  it("prevents installation while transfers are running and handles unsupported platforms", async () => {
    mockedInvoke
      .mockResolvedValueOnce(available)
      .mockResolvedValueOnce({ supported: false, update: null });
    render(<Harness active={2} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(
      screen.getByRole("button", { name: "Install update…" }),
    ).toBeDisabled();
    expect(
      screen.getByText(/Pause or cancel all transfers/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(screen.getByText(/Windows x64 installers/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Install update…" }),
    ).not.toBeInTheDocument();
  });
  it("checks after startup without installing, reports an offline error, and lets the user retry", async () => {
    vi.useFakeTimers();
    mockedInvoke
      .mockRejectedValueOnce(new Error("Cannot reach GitHub Releases"))
      .mockResolvedValueOnce({ supported: true, update: null });
    render(<Harness automatic />);
    expect(mockedInvoke).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTime(3000));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Cannot reach GitHub Releases",
    );
    vi.useRealTimers();
    await userEvent.click(
      screen.getByRole("button", { name: "Check for updates" }),
    );
    expect(screen.getByRole("status")).toHaveTextContent("You are up to date");
    expect(
      mockedInvoke.mock.calls.every(
        ([command]) => command === "check_for_updates",
      ),
    ).toBe(true);
  });
  it("keeps Settings open and prevents saving while the installer operation is running", async () => {
    mockedInvoke.mockImplementation(async (command, args) => {
      if (command === "check_for_updates") return available;
      if (command === "install_update") return new Promise(() => {});
      if (
        command === "dispatch" &&
        (args as { request: { action: string } }).request.action ===
          "transfer_presets"
      )
        return [];
      if (command === "dispatch")
        return {
          concurrent_files: 2,
          concurrent_parts: 3,
          max_requests: 6,
          part_size: 16777216,
          multipart_threshold: 33554432,
          retries: 3,
        };
      throw new Error(`Unexpected command: ${command}`);
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <QueryClientProvider client={client}>
        <Harness settings />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    await user.click(screen.getByRole("button", { name: "Install update…" }));
    await user.click(
      screen.getByRole("button", { name: "Install and restart" }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Close dialog" }),
      ).toBeDisabled(),
    );
    expect(
      screen.getByRole("button", { name: "Save settings" }),
    ).toBeDisabled();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(close).not.toHaveBeenCalled();
  });
});
