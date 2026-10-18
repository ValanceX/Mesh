// Builds the runtime's WebAssembly module with Cargo and puts it where
// the package ships it, `dist/mesh-runtime.wasm`. Also builds the
// test-only module, with the `test-hooks` feature, as
// `test/build/mesh-runtime-test.wasm`; it never ships.
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
      "mesh-runtime-wasm",
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
  return join(targetDir, "wasm32-unknown-unknown", "wasm", "mesh_runtime_wasm.wasm");
}

// The linked module is then optimized (decisions W23a, W26): binaryen's `-O2` with inlining switched off. What a
// site ships is this module compressed with brotli, and that decides the flags, not the file's size or its gzip:
// on this module `-Oz` makes the file 28 percent smaller and its gzip 5 percent smaller and its brotli 1.4 percent
// LARGER, plain `-O2` and `-Oz` inline code the compressor had already folded, and `-O2` without inlining gives
// 181,837 -> 176,389 bytes brotli (-3.0 percent), 248,984 -> 234,818 gzip (-5.7 percent), 817,965 -> 700,870 raw.
// `-all` because the module uses what rustc's current wasm32 target uses. `MESH_NO_WASM_OPT=1` skips it, for a
// build that must be the linker's output.
function optimize(from, to) {
  if (process.env.MESH_NO_WASM_OPT) {
    copyFileSync(from, to);
    return;
  }
  const binaryen = join(dirname(createRequire(import.meta.url).resolve("binaryen/package.json")), "bin", "wasm-opt");
  execFileSync(
    process.execPath,
    [binaryen, "-all", "-O2", "-fimfs", "0", "-ocimfs", "0", "-aimfs", "0", from, "-o", to],
    { stdio: "inherit" },
  );
}

const shipped = build(join(root, "target"), []);
mkdirSync(join(here, "..", "dist"), { recursive: true });
optimize(shipped, join(here, "..", "dist", "mesh-runtime.wasm"));

// A separate target directory, so the two builds don't overwrite each other.
const hooks = build(join(root, "target", "runtime-test-hooks"), ["--features", "test-hooks"]);
mkdirSync(join(here, "..", "test", "build"), { recursive: true });
copyFileSync(hooks, join(here, "..", "test", "build", "mesh-runtime-test.wasm"));
