import type { FileEntry } from "./api";

export interface FileOrder {
  search: string;
  sort: "name" | "size" | "modified";
  descending: boolean;
}
export interface OrderedFiles {
  indices: Uint32Array;
  bytes: number;
}
export function orderFiles(
  entries: FileEntry[],
  order: FileOrder,
): OrderedFiles {
  const search = order.search.toLocaleLowerCase();
  const indices: number[] = [];
  let bytes = 0;
  entries.forEach((entry, index) => {
    if (entry.name.toLocaleLowerCase().includes(search)) {
      indices.push(index);
      if (!entry.is_dir) bytes += entry.size;
    }
  });
  indices.sort((left, right) => {
    const a = entries[left],
      b = entries[right];
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    const comparison =
      order.sort === "name"
        ? a.name.localeCompare(b.name)
        : (a[order.sort] ?? 0) - (b[order.sort] ?? 0);
    return order.descending ? -comparison : comparison;
  });
  return { indices: Uint32Array.from(indices), bytes };
}
