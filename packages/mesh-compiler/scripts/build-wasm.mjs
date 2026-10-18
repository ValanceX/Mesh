// Builds the WebAssembly module with Cargo (outline v0.4 D1) and puts it
// where the package ships it, `dist/mesh.wasm`. Also builds the test-only
// module, with the `test-hooks` feature, as `test/build/mesh-test.wasm`;
// it never ships.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const cargo = process.env.CARGO ?? "cargo";

function build(targetDir, features) {
  execFileSync(
    cargo,
    [
      "build",
      "--locked",
      "-p",
      "mesh-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--profile",
      "wasm",
      "--target-dir",
      targetDir,
      ...features,
    ],
    { cwd: root, stdio: "inherit" },
  );
  return join(targetDir, "wasm32-unknown-unknown", "wasm", "mesh_wasm.wasm");
}

// The linked module is then optimized for what the budget counts, its gzipped size (W23a, the size analysis):
// binaryen's `-Oz` with inlining switched off. Plain `-Oz` makes the file a fifth smaller and its gzip a fifth of a
// percent larger (it inlines code that gzip had already folded); without inlining the gzip is 6 percent smaller.
// `-all` because the module uses what rustc's current wasm32 target uses (bulk memory, sign extension, ...).
// `MESH_NO_WASM_OPT=1` skips it, for a build that must be the linker's output.
function optimize(from, to) {
  if (process.env.MESH_NO_WASM_OPT) {
    copyFileSync(from, to);
    return;
  }
  const binaryen = join(dirname(createRequire(import.meta.url).resolve("binaryen/package.json")), "bin", "wasm-opt");
  execFileSync(
    process.execPath,
    [binaryen, "-all", "-Oz", "-fimfs", "0", "-ocimfs", "0", "-aimfs", "0", from, "-o", to],
    { stdio: "inherit" },
  );
}

const shipped = build(join(root, "target"), []);
mkdirSync(join(here, "..", "dist"), { recursive: true });
optimize(shipped, join(here, "..", "dist", "mesh.wasm"));

// A separate target directory, so the two builds don't overwrite each other.
const hooks = build(join(root, "target", "wasm-test-hooks"), ["--features", "test-hooks"]);
mkdirSync(join(here, "..", "test", "build"), { recursive: true });
copyFileSync(hooks, join(here, "..", "test", "build", "mesh-test.wasm"));
