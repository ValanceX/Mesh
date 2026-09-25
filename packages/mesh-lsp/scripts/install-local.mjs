// Stages a native `mesh-lsp` into this platform's package, packs that
// package and the launcher, and installs both tarballs, offline, into an
// empty project: what a user's `npm install` gives, without a registry.
// CI runs it on each target's own runner (outline v0.4 D9).
//
//   node scripts/install-local.mjs <binary> <out-dir>
//
// Prints a JSON object: the tarballs, the project, the installed
// launcher's `dist/cli.js`, and npm's `.bin` shim.
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [binary, out] = process.argv.slice(2).map((arg) => resolve(arg));
if (!binary || !out) {
  throw new Error("usage: node scripts/install-local.mjs <binary> <out-dir>");
}
const here = dirname(fileURLToPath(import.meta.url));
const packages = join(here, "..", "..");
const key = `${process.platform}-${process.arch}`;
const exe = process.platform === "win32" ? "mesh-lsp.exe" : "mesh-lsp";
const shell = process.platform === "win32";
const npm = shell ? "npm.cmd" : "npm";

const platform = join(packages, `mesh-lsp-${key}`);
mkdirSync(join(platform, "bin"), { recursive: true });
copyFileSync(binary, join(platform, "bin", exe));
chmodSync(join(platform, "bin", exe), 0o755);

const tarballs = join(out, "tarballs");
mkdirSync(tarballs, { recursive: true });
const pack = (dir) =>
  join(
    tarballs,
    JSON.parse(execFileSync(npm, ["pack", "--json", "--pack-destination", tarballs], { cwd: dir, encoding: "utf8", shell }))[0]
      .filename,
  );
const launcherTarball = pack(join(packages, "mesh-lsp"));
const platformTarball = pack(platform);

const project = join(out, "project");
mkdirSync(project, { recursive: true });
writeFileSync(join(project, "package.json"), JSON.stringify({ name: "project", private: true }));
execFileSync(npm, ["install", "--offline", "--no-audit", "--no-fund", launcherTarball, platformTarball], {
  cwd: project,
  stdio: "inherit",
  shell,
});

process.stdout.write(
  JSON.stringify({
    launcherTarball,
    platformTarball,
    project,
    launcher: join(project, "node_modules", "@valancex", "mesh-lsp", "dist", "cli.js"),
    shim: join(project, "node_modules", ".bin", shell ? "mesh-lsp.cmd" : "mesh-lsp"),
  }) + "\n",
);
