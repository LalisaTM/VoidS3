import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, expect, it, vi } from "vitest";
import { QueuePanel } from "../components/QueuePanel";
import {
  api,
  emptyQueueSummary,
  type QueueSummary,
  type Transfer,
} from "../lib/api";
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  desktop: true,
  api: vi.fn(),
}));
const mockApi = vi.mocked(api);
const job: Transfer = {
  id: "conflict",
  profile_id: "p",
  local_path: "C:/photo.jpg",
  key: "photo.jpg",
  direction: "upload",
  size: 100,
  bytes_done: 0,
  created: 0,
  status: "conflict",
  error: null,
  upload_id: null,
  batch_id: "batch",
  conflict: { size: 90, modified: null, can_replace: true },
};
const summary: QueueSummary = {
  ...emptyQueueSummary,
  total: 10_000,
  active: 9900,
  counts: { conflict: 100 },
  cancellable: 10_000,
};
function mount(open = true) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <QueuePanel
        summary={summary}
        open={open}
        toggle={vi.fn()}
        control={vi.fn()}
        clear={vi.fn()}
        bulkControl={vi.fn()}
      />
    </QueryClientProvider>,
  );
}
beforeEach(() => {
  mockApi.mockReset();
  mockApi.mockImplementation(async (action, args) => {
    if (action === "transfer_page")
      return {
        total: 10_000,
        offset: args?.offset ?? 0,
        items: Array.from({ length: 100 }, (_, index) => ({
          ...job,
          id: `job-${index}`,
        })),
      };
    if (action === "resolve_upload") return null;
    throw new Error(`Unexpected action ${action}`);
  });
});
it("renders only one bounded page for a 10,000-file queue and requests another page on demand", async () => {
  const { container } = mount();
  await waitFor(() =>
    expect(container.querySelectorAll(".transfer-row")).toHaveLength(100),
  );
  expect(mockApi).toHaveBeenCalledWith("transfer_page", {
    filter: "all",
    offset: 0,
    limit: 100,
  });
  expect(screen.getByText(/of 10[.,]000 in this view/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Next page" }));
  await waitFor(() =>
    expect(mockApi).toHaveBeenCalledWith("transfer_page", {
      filter: "all",
      offset: 100,
      limit: 100,
    }),
  );
  expect(container.querySelectorAll(".transfer-row")).toHaveLength(100);
  await userEvent.selectOptions(
    screen.getByRole("combobox", { name: "Filter transfers" }),
    "conflict",
  );
  await waitFor(() =>
    expect(mockApi).toHaveBeenCalledWith("transfer_page", {
      filter: "conflict",
      offset: 0,
      limit: 100,
    }),
  );
});
it("does not fetch queue pages when collapsed", () => {
  mount(false);
  expect(mockApi).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Cancel all" })).toBeEnabled();
});
it("resolves a queue conflict with a backend batch policy", async () => {
  mockApi.mockResolvedValueOnce({ total: 1, offset: 0, items: [job] });
  mount();
  await userEvent.click(await screen.findByRole("button", { name: "Resolve" }));
  await userEvent.click(screen.getByRole("checkbox"));
  await userEvent.click(screen.getByRole("button", { name: "Keep both" }));
  await waitFor(() =>
    expect(mockApi).toHaveBeenCalledWith("resolve_upload", {
      id: "conflict",
      decision: "keep_both",
      all: true,
    }),
  );
});
