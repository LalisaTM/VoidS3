import { invoke, isTauri } from "@tauri-apps/api/core";
export const desktop = isTauri();
export async function api<T>(
  action: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!desktop)
    throw new Error(
      "Open the VoidS3 desktop app to access your filesystem, keychain, and S3 connections. This browser view has no storage access.",
    );
  return invoke<T>("dispatch", { request: { action, ...args } });
}
export interface Profile {
  id: string;
  name: string;
  provider: string;
  endpoint: string;
  region: string;
  bucket: string;
  prefix: string;
  path_style: boolean;
  read_only: boolean;
  allow_http: boolean;
}
export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number | null;
  etag: string | null;
  storage_class?: string | null;
}
export interface Page {
  entries: FileEntry[];
  next_token: string | null;
}
export interface Transfer {
  id: string;
  profile_id: string;
  key: string;
  local_path: string;
  direction: "upload" | "download";
  size: number;
  bytes_done: number;
  status: string;
  error: string | null;
  created: number;
  upload_id: string | null;
}
export interface TransferSettings {
  concurrent_files: number;
  concurrent_parts: number;
  max_requests: number;
  part_size: number;
  multipart_threshold: number;
  retries: number;
}
export type BatchOperation = "pause" | "resume" | "cancel";
export interface BatchControlResult {
  requested: number;
  failed: number;
  errors: string[];
}
export interface DeleteResult {
  deleted: string[];
  failed: number;
  errors: string[];
}
export type ConflictAction = "replace" | "skip" | "keep_both";
export interface UploadChoice {
  index: number;
  action: ConflictAction;
}
export interface UploadConflict {
  index: number;
  path: string;
  key: string;
  source: { size: number; modified: number | null };
  destination: { size: number; modified: number | null };
  can_replace: boolean;
}
export interface PreparedUpload {
  id: string;
  total: number;
  conflicts: UploadConflict[];
}
export interface UploadResult {
  queued: string[];
  skipped: number;
  failed: number;
  errors: string[];
}
export interface Change {
  key: string;
  action: "upload" | "download" | "conflict" | "skip";
  size: number;
  reason: string;
}
export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
export function bytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "-";
  if (n < 1024) return `${n} B`;
  const i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), 4);
  return `${(n / 1024 ** i).toFixed(n / 1024 ** i < 10 ? 1 : 0)} ${["B", "KiB", "MiB", "GiB", "TiB"][i]}`;
}
export function parent(path: string, remote = false): string {
  if (remote) {
    const parts = path.replace(/\/$/, "").split("/");
    parts.pop();
    return parts.length ? `${parts.join("/")}/` : "";
  }
  const slash = path.includes("\\") ? "\\" : "/";
  const trimmed = path.replace(/[\\/]$/, "");
  const index = trimmed.lastIndexOf(slash);
  if (index <= 0) return slash;
  const result = trimmed.slice(0, index);
  return /^[A-Za-z]:$/.test(result) ? `${result}\\` : result;
}
