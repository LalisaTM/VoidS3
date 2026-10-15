import { test } from "node:test";
import assert from "node:assert/strict";
import { createManifest } from "./updater-manifest.mjs";
const input = {
  version: "0.2.0",
  tag: "v0.2.0",
  installer: "VoidS3_0.2.0_x64-setup.exe",
  // Manifest structure only. Cryptographic verification is tested in Rust.
  signature: Buffer.from(
    "untrusted comment: synthetic\nnot-a-real-signature\ntrusted comment: synthetic\tversion:0.2.0\nnot-a-real-signature",
  ).toString("base64"),
  notes: "Test release notes",
  date: "2026-10-15T00:00:00Z",
};
test("manifest points to an immutable, versioned GitHub release asset", () => {
  const manifest = createManifest(input);
  assert.equal(manifest.version, input.version);
  assert.equal(manifest.platforms["windows-x86_64"].signature, input.signature);
  assert.equal(
    manifest.platforms["windows-x86_64"].url,
    "https://github.com/LalisaTM/VoidS3/releases/download/v0.2.0/VoidS3_0.2.0_x64-setup.exe",
  );
});
test("invalid versions, mismatched tags, paths, dates, and signature URLs fail closed", () => {
  for (const patch of [
    { version: "bad" },
    { tag: "v0.1.1" },
    { installer: "../VoidS3-setup.exe" },
    { installer: "wrong.zip" },
    { date: "bad" },
    { signature: "https://example.invalid/signature.sig" },
  ])
    assert.throws(() => createManifest({ ...input, ...patch }));
});
