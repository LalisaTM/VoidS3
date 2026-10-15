import { readFileSync, writeFileSync } from "node:fs";
const files = [
  "package.json",
  "apps/desktop/package.json",
  "apps/desktop/src-tauri/tauri.conf.json",
];
const cargoPath = "Cargo.toml";
const cargo = readFileSync(cargoPath, "utf8");
const current = cargo.match(/\[workspace\.package\]\s+version = "([^"]+)"/)[1];
const flag = process.argv[2];
if (flag === "--check") {
  for (const file of files)
    if (JSON.parse(readFileSync(file, "utf8")).version !== current)
      throw new Error(`Version mismatch in ${file}`);
  const lock = JSON.parse(readFileSync("package-lock.json", "utf8"));
  for (const key of ["", "apps/desktop"])
    if (lock.packages[key].version !== current)
      throw new Error(`Lockfile version mismatch: ${key}`);
  const cargoLock = readFileSync("Cargo.lock", "utf8");
  for (const match of cargoLock.matchAll(
    /name = "(voids3-[^"]+)"\r?\nversion = "([^"]+)"/g,
  )) {
    if (match[2] !== current)
      throw new Error(`Cargo.lock version mismatch: ${match[1]}`);
  }
  const tag =
    process.env.GITHUB_REF_TYPE === "tag"
      ? process.env.GITHUB_REF_NAME
      : undefined;
  if (tag && tag !== `v${current}`)
    throw new Error(`Tag ${tag} does not match v${current}`);
  console.log(`Versions consistent: ${current}`);
} else {
  if (!/^\d+\.\d+\.\d+(?:-[a-z0-9.-]+)?$/.test(flag ?? ""))
    throw new Error("Usage: node scripts/version.mjs --check | <semver>");
  for (const file of files) {
    const data = JSON.parse(readFileSync(file, "utf8"));
    data.version = flag;
    writeFileSync(file, `${JSON.stringify(data, null, 2)}\n`);
  }
  writeFileSync(
    cargoPath,
    cargo.replace(`version = "${current}"`, `version = "${flag}"`),
  );
  console.log(
    `Set version ${flag}. Run npm install --package-lock-only and cargo check, update CHANGELOG.md, and commit the synchronized lockfiles.`,
  );
}
