// Publishes the seven packed npm packages of a MESH release (outline v0.4
// D10), from the tarballs the release workflow built on its runners.
//
//   node .github/scripts/publish.mjs <tarball-dir> [--publish]
//
// Without --publish it's a dry run: every check, and `npm publish
// --dry-run`. It checks that the tag (GITHUB_REF_NAME, on a tag push) is
// the workspace version, that there is exactly one tarball per package at
// that version, and then publishes the platform packages first, then the
// launcher that depends on them, then the compiler. A version the registry
// already has is skipped, so a failed run can be run again.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [dir, flag] = process.argv.slice(2);
const publish = flag === "--publish";
if (!dir || (flag && !publish)) {
  throw new Error("usage: node .github/scripts/publish.mjs <tarball-dir> [--publish]");
}

function workspaceVersion() {
  let inPackage = false;
  for (const raw of readFileSync("Cargo.toml", "utf8").split("\n")) {
    const line = raw.trim();
    if (line.startsWith("[")) {
      inPackage = line === "[workspace.package]";
    } else if (inPackage && line.startsWith("version = ")) {
      return line.slice("version = ".length).replaceAll('"', "");
    }
  }
  throw new Error("Cargo.toml has no [workspace.package] version");
}

const version = workspaceVersion();
if (process.env.GITHUB_REF_TYPE === "tag") {
  const tag = process.env.GITHUB_REF_NAME;
  if (tag !== `v${version}`) {
    throw new Error(`the tag is ${tag}, but the workspace version is ${version}: tag v${version}`);
  }
}
if (publish && process.env.GITHUB_REF_TYPE !== "tag") {
  throw new Error("publishing needs a tag: push v" + version);
}

// Dependencies first: the launcher's optional dependencies, then it.
const PACKAGES = [
  "mesh-lsp-linux-x64",
  "mesh-lsp-linux-arm64",
  "mesh-lsp-darwin-x64",
  "mesh-lsp-darwin-arm64",
  "mesh-lsp-win32-x64",
  "mesh-lsp",
  "mesh-compiler",
];

const tarballs = PACKAGES.map((name) => {
  const tarball = join(dir, `valancex-${name}-${version}.tgz`);
  if (!existsSync(tarball)) {
    throw new Error(`no ${tarball}: every package must be packed at ${version}`);
  }
  return { name: `@valancex/${name}`, tarball };
});

function published(name) {
  try {
    execFileSync("npm", ["view", `${name}@${version}`, "version"], { stdio: ["ignore", "pipe", "ignore"] });
    return true;
  } catch {
    return false;
  }
}

for (const { name, tarball } of tarballs) {
  if (publish && published(name)) {
    console.log(`${name}@${version} is already published; skipped`);
    continue;
  }
  const args = ["publish", tarball, "--access", "public"];
  args.push(...(publish ? ["--provenance"] : ["--dry-run"]));
  execFileSync("npm", args, { stdio: "inherit" });
  console.log(`${publish ? "published" : "dry run:"} ${name}@${version}`);
}
