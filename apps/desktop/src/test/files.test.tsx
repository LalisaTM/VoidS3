import { useState } from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import { Files } from "../components/Files";
import type { FileEntry } from "../lib/api";
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
const entries: FileEntry[] = [
  {
    name: "beta.txt",
    path: "beta.txt",
    is_dir: false,
    size: 300,
    modified: 0,
    etag: null,
  },
  {
    name: "alpha.json",
    path: "alpha.json",
    is_dir: false,
    size: 50,
    modified: 0,
    etag: null,
  },
  {
    name: "assets",
    path: "assets/",
    is_dir: true,
    size: 0,
    modified: 0,
    etag: null,
  },
];
function Harness() {
  const [selected, setSelected] = useState<string[]>([]);
  return (
    <Files
      entries={entries}
      selected={selected}
      onSelect={setSelected}
      onOpen={vi.fn()}
      loading={false}
      connected
      remote
      writable={false}
      onAction={vi.fn()}
      onRetry={vi.fn()}
    />
  );
}
describe("file navigation", () => {
  it("sorts folders first, supports range selection, and filters only loaded entries", async () => {
    render(<Harness />);
    const rows = screen.getAllByRole("option");
    expect(rows[0]).toHaveTextContent("assets");
    expect(rows[1]).toHaveTextContent("alpha.json");
    fireEvent.click(rows[0]);
    fireEvent.click(rows[2], { shiftKey: true });
    expect(screen.getAllByRole("option", { selected: true })).toHaveLength(3);
    await userEvent.type(
      screen.getByRole("textbox", { name: "Filter remote files" }),
      ".json",
    );
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option")).toHaveTextContent("alpha.json");
  });
  it("supports keyboard selection and disables read-only folder creation", () => {
    render(<Harness />);
    const list = screen.getByRole("listbox");
    fireEvent.keyDown(list, { key: "ArrowDown" });
    expect(screen.getByRole("option", { selected: true })).toHaveTextContent(
      "assets",
    );
    fireEvent.keyDown(list, { key: "End" });
    expect(screen.getByRole("option", { selected: true })).toHaveTextContent(
      "beta.txt",
    );
    expect(
      screen.getByRole("button", { name: "New remote folder" }),
    ).toBeDisabled();
  });
});
