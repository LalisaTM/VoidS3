import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

export function createManifest({
  version,
  tag,
  installer,
  signature,
  notes,
  date,
}) {
  if (
    !/^\d+\.\d+\.\d+(?:-[a-z0-9.-]+)?$/.test(version) ||
    tag !== `v${version}`
  )
    throw new Error("Update tag must match the package version");
  if (
    basename(installer) !== installer ||
    !/^[A-Za-z0-9_.-]+-setup\.exe$/.test(installer)
  )
    throw new Error("Expected one Windows NSIS installer filename");
  const decoded = Buffer.from(signature.trim(), "base64").toString("utf8");
  if (
    !decoded.startsWith("untrusted comment:") ||
    !decoded.includes("trusted comment:")
  )
    throw new Error("Expected the contents of a Tauri signature file");
  if (
    !decoded
      .split("\n")
      .some(
        (line) =>
          line.startsWith("trusted comment:") &&
          line.split("\t").includes(`version:${version}`),
      )
  )
    throw new Error("Updater signature must bind the announced version");
  if (!Number.isFinite(Date.parse(date)))
    throw new Error("Invalid publication date");
  return {
    version,
    notes,
    pub_date: new Date(date).toISOString(),
    platforms: {
      "windows-x86_64": {
        signature: signature.trim(),
        url: `https://github.com/LalisaTM/VoidS3/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(installer)}`,
      },
    },
  };
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const directory = process.argv[2];
  if (!directory)
    throw new Error(
      "Usage: node scripts/updater-manifest.mjs <release-assets-directory>",
    );
  const installers = readdirSync(directory).filter((name) =>
    name.endsWith("-setup.exe"),
  );
  if (installers.length !== 1)
    throw new Error("Expected exactly one updater installer");
  const { version } = JSON.parse(readFileSync("package.json", "utf8"));
  const manifest = createManifest({
    version,
    tag: process.env.GITHUB_REF_NAME,
    installer: installers[0],
    signature: readFileSync(join(directory, `${installers[0]}.sig`), "utf8"),
    notes: readFileSync("docs/release-notes.md", "utf8"),
    date: new Date().toISOString(),
  });
  writeFileSync(
    join(directory, "latest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}
