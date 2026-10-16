import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import { SettingsDialog } from "../components/SettingsDialog";
import { api, type TransferSettings } from "../lib/api";
import type { Updater } from "../lib/useUpdater";
vi.mock("../lib/api", async (original) => ({
  ...(await original<typeof import("../lib/api")>()),
  api: vi.fn(),
}));
const balanced: TransferSettings = {
  concurrent_files: 2,
  concurrent_parts: 3,
  max_requests: 6,
  part_size: 16777216,
  multipart_threshold: 33554432,
  retries: 3,
};
it("loads backend presets, explains them, supports custom edits and saves their actual values", async () => {
  const small = { ...balanced, concurrent_files: 12, max_requests: 16 };
  vi.mocked(api).mockImplementation(async (action) => {
    if (action === "settings") return balanced;
    if (action === "transfer_presets")
      return [
        {
          id: "balanced",
          name: "Balanced",
          description: "Mixed files",
          settings: balanced,
        },
        {
          id: "many_small",
          name: "Many small files",
          description: "More files in parallel",
          settings: small,
        },
      ];
    return null;
  });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <SettingsDialog
        onClose={vi.fn()}
        activeTransfers={0}
        updater={
          {
            busy: false,
            state: { phase: "idle" },
            check: vi.fn(),
          } as unknown as Updater
        }
      />
    </QueryClientProvider>,
  );
  const preset = await screen.findByRole("combobox", {
    name: "Transfer preset",
  });
  await waitFor(() => expect(preset).toHaveValue("balanced"));
  await userEvent.selectOptions(preset, "many_small");
  expect(screen.getByRole("status")).toHaveTextContent(
    "More files in parallel",
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Advanced transfer settings" }),
  );
  const files = screen.getByRole("spinbutton", { name: "Concurrent files" });
  expect(files).toHaveValue(12);
  await userEvent.clear(files);
  await userEvent.type(files, "8");
  expect(preset).toHaveValue("custom");
  await userEvent.click(screen.getByRole("button", { name: "Save settings" }));
  await waitFor(() =>
    expect(api).toHaveBeenCalledWith("save_settings", {
      settings: { ...small, concurrent_files: 8 },
    }),
  );
});
