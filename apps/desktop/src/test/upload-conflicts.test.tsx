import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UploadConflictDialog } from "../components/UploadConflictDialog";
import { useUpload } from "../lib/useUpload";
import { api, type PreparedUpload, type UploadResult } from "../lib/api";

vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  api: vi.fn(),
}));
const mockApi = vi.mocked(api);
const complete = vi.fn();
const error = vi.fn();
const plan: PreparedUpload = {
  id: "batch-1",
  total: 4,
  conflicts: [0, 1, 3].map((index) => ({
    index,
    path: `C:/photos/holiday-${index}.jpg`,
    key: `albums/holiday-${index}.jpg`,
    source: { size: 1024, modified: 1700000000 },
    destination: { size: 2048, modified: 1600000000 },
    can_replace: true,
  })),
};
const result: UploadResult = {
  queued: ["job-1"],
  skipped: 1,
  failed: 0,
  errors: [],
};
function Harness() {
  const uploads = useUpload({ onComplete: complete, onError: error });
  return (
    <>
      <button
        onClick={() =>
          void uploads
            .start("connection", ["C:/photos"], "albums/")
            .catch(error)
        }
      >
        Upload folder
      </button>
      <output>{uploads.phase}</output>
      {uploads.plan && uploads.phase === "confirming" && (
        <UploadConflictDialog
          key={uploads.plan.id}
          plan={uploads.plan}
          onResolve={uploads.resolve}
          onCancel={() => void uploads.cancel()}
        />
      )}
      {uploads.phase === "checking" && (
        <button onClick={() => void uploads.cancel()}>Cancel check</button>
      )}
    </>
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  mockApi.mockImplementation(async (action) => {
    if (action === "prepare_upload") return plan;
    if (action === "commit_upload") return result;
    if (action === "discard_upload") return null;
    throw new Error(`Unexpected action ${action}`);
  });
});
describe("upload conflict decisions", () => {
  it("compares both files and waits for every decision before queuing", async () => {
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Upload folder" }));
    expect(mockApi).toHaveBeenCalledWith("prepare_upload", {
      id: "connection",
      paths: ["C:/photos"],
      prefix: "albums/",
    });
    expect(
      screen.getByRole("region", { name: "Incoming file" }),
    ).toHaveTextContent("1.0 KiB");
    expect(
      screen.getByRole("region", { name: "Existing file" }),
    ).toHaveTextContent("2.0 KiB");
    await user.click(screen.getByRole("button", { name: "Replace" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("Conflict 2 of 3");
    await user.click(screen.getByRole("button", { name: "Skip" }));
    expect(mockApi).not.toHaveBeenCalledWith(
      "commit_upload",
      expect.anything(),
    );
    await user.click(screen.getByRole("button", { name: "Keep both" }));
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("commit_upload", {
        id: plan.id,
        choices: [
          { index: 0, action: "replace" },
          { index: 1, action: "skip" },
          { index: 3, action: "keep_both" },
        ],
      }),
    );
    expect(complete).toHaveBeenCalledWith(result);
  });
  it.each([
    ["Replace", "replace"],
    ["Skip", "skip"],
    ["Keep both", "keep_both"],
  ])(
    "applies %s only to this batch's remaining conflicts",
    async (label, action) => {
      render(<Harness />);
      const user = userEvent.setup();
      await user.click(screen.getByRole("button", { name: "Upload folder" }));
      await user.click(screen.getByRole("checkbox"));
      await user.click(screen.getByRole("button", { name: label }));
      await waitFor(() =>
        expect(mockApi).toHaveBeenCalledWith("commit_upload", {
          id: plan.id,
          choices: plan.conflicts.map(({ index }) => ({ index, action })),
        }),
      );
      await user.click(screen.getByRole("button", { name: "Upload folder" }));
      expect(screen.getByRole("dialog")).toHaveTextContent("Conflict 1 of 3");
      expect(screen.getByRole("checkbox")).not.toBeChecked();
    },
  );
  it("cancels the entire unqueued batch after earlier choices and supports Escape", async () => {
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Upload folder" }));
    await user.click(screen.getByRole("button", { name: "Replace" }));
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("discard_upload", { id: plan.id }),
    );
    expect(mockApi).not.toHaveBeenCalledWith(
      "commit_upload",
      expect.anything(),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(complete).not.toHaveBeenCalled();
  });
  it("disables replacement if a destination ETag is unavailable", async () => {
    const noEtag = {
      ...plan,
      conflicts: [{ ...plan.conflicts[0], can_replace: false }],
    };
    render(
      <UploadConflictDialog
        plan={noEtag}
        onResolve={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Replace" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Keep both" })).toBeEnabled();
  });
  it("queues immediately when there are no conflicts", async () => {
    mockApi
      .mockResolvedValueOnce({ ...plan, conflicts: [] })
      .mockResolvedValueOnce(result);
    render(<Harness />);
    await userEvent.click(
      screen.getByRole("button", { name: "Upload folder" }),
    );
    expect(mockApi).toHaveBeenCalledWith("commit_upload", {
      id: plan.id,
      choices: [],
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("discards a cancelled in-flight preflight without committing or starting a second batch", async () => {
    let finish!: (value: PreparedUpload) => void;
    mockApi.mockReturnValueOnce(
      new Promise<PreparedUpload>((resolve) => {
        finish = resolve;
      }),
    );
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Upload folder" }));
    await user.click(screen.getByRole("button", { name: "Cancel check" }));
    fireEvent.click(screen.getByRole("button", { name: "Upload folder" }));
    await waitFor(() => expect(error).toHaveBeenCalled());
    finish(plan);
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith("discard_upload", { id: plan.id }),
    );
    expect(
      mockApi.mock.calls.filter(([action]) => action === "prepare_upload"),
    ).toHaveLength(1);
    expect(mockApi).not.toHaveBeenCalledWith(
      "commit_upload",
      expect.anything(),
    );
  });
  it("reports a failed consumed confirmation and requires fresh preflight", async () => {
    mockApi
      .mockResolvedValueOnce({ ...plan, conflicts: [] })
      .mockRejectedValueOnce(new Error("Source changed"));
    render(<Harness />);
    await userEvent.click(
      screen.getByRole("button", { name: "Upload folder" }),
    );
    await waitFor(() =>
      expect(error).toHaveBeenCalledWith(
        expect.stringContaining("Source changed"),
      ),
    );
    expect(complete).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Upload folder" })).toBeEnabled();
  });
});
