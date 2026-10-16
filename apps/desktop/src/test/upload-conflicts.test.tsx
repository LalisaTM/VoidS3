import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UploadConflictDialog } from "../components/UploadConflictDialog";
import { useUpload } from "../lib/useUpload";
import { api, type Transfer } from "../lib/api";
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  api: vi.fn(),
}));
const mockApi = vi.mocked(api);
const started = vi.fn();
const failed = vi.fn();
export const conflict: Transfer = {
  id: "job-1",
  profile_id: "connection",
  key: "images/photo.jpg",
  local_path: "C:/photos/photo.jpg",
  direction: "upload",
  size: 1024,
  bytes_done: 0,
  status: "conflict",
  error: null,
  created: 0,
  upload_id: null,
  batch_id: "batch-1",
  source_modified: 1700000000,
  conflict: { size: 2048, modified: 1600000000, can_replace: true },
};
function Harness() {
  const uploads = useUpload({ onStarted: started });
  return (
    <button
      disabled={uploads.starting}
      onClick={() =>
        void uploads.start("connection", ["C:/photos"], "images/").catch(failed)
      }
    >
      Upload folder
    </button>
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  mockApi.mockReset();
  mockApi.mockResolvedValue("batch-1");
});
describe("background uploads", () => {
  it("registers background discovery without waiting for remote preflight or opening a modal", async () => {
    render(<Harness />);
    await userEvent.click(
      screen.getByRole("button", { name: "Upload folder" }),
    );
    expect(mockApi).toHaveBeenCalledExactlyOnceWith("start_upload", {
      id: "connection",
      paths: ["C:/photos"],
      prefix: "images/",
    });
    expect(started).toHaveBeenCalledWith("batch-1");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("prevents duplicate starts while registration is pending and recovers from errors", async () => {
    let reject!: (error: Error) => void;
    mockApi.mockReturnValueOnce(
      new Promise((_resolve, fail) => {
        reject = fail;
      }),
    );
    render(<Harness />);
    await userEvent.click(screen.getByRole("button"));
    expect(screen.getByRole("button")).toBeDisabled();
    reject(new Error("Offline"));
    await waitFor(() => expect(screen.getByRole("button")).toBeEnabled());
    expect(failed).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button"));
    expect(started).toHaveBeenCalledWith("batch-1");
  });
});
describe("queued upload conflicts", () => {
  it.each([
    ["Replace", "replace"],
    ["Skip", "skip"],
    ["Keep both", "keep_both"],
  ])(
    "resolves %s with an optional batch policy covering future conflicts",
    async (label, action) => {
      const resolve = vi.fn();
      render(
        <UploadConflictDialog
          job={conflict}
          onResolve={resolve}
          onClose={vi.fn()}
        />,
      );
      expect(
        screen.getByRole("region", { name: "Incoming file" }),
      ).toHaveTextContent("1.0 KiB");
      expect(
        screen.getByRole("region", { name: "Existing file" }),
      ).toHaveTextContent("2.0 KiB");
      expect(screen.getByRole("dialog")).toHaveTextContent(
        "Other files continue",
      );
      await userEvent.click(screen.getByRole("checkbox"));
      expect(screen.getByRole("checkbox")).toHaveAccessibleName(
        /including files discovered later/,
      );
      await userEvent.click(screen.getByRole("button", { name: label }));
      expect(resolve).toHaveBeenCalledWith(action, true);
    },
  );
  it("keeps decisions scoped to one file by default and lets the user decide later", async () => {
    const resolve = vi.fn(),
      close = vi.fn();
    render(
      <UploadConflictDialog
        job={conflict}
        onResolve={resolve}
        onClose={close}
      />,
    );
    expect(screen.getByRole("checkbox")).not.toBeChecked();
    await userEvent.keyboard("{Escape}");
    expect(close).toHaveBeenCalledOnce();
    expect(resolve).not.toHaveBeenCalled();
  });
  it("disables replacement without an ETag and blocks closing during submission", async () => {
    const close = vi.fn();
    render(
      <UploadConflictDialog
        job={{
          ...conflict,
          conflict: { size: 2, modified: null, can_replace: false },
        }}
        onResolve={vi.fn()}
        onClose={close}
        busy
      />,
    );
    expect(screen.getByRole("button", { name: "Replace" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Decide later" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    expect(close).not.toHaveBeenCalled();
  });
});
