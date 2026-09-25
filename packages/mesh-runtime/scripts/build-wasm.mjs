// Builds the runtime's WebAssembly module with Cargo and puts it where
// the package ships it, `dist/mesh-runtime.wasm`. Also builds the
// test-only module, with the `test-hooks` feature, as
// `test/build/mesh-runtime-test.wasm`; it never ships.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
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

const shipped = build(join(root, "target"), []);
mkdirSync(join(here, "..", "dist"), { recursive: true });
copyFileSync(shipped, join(here, "..", "dist", "mesh-runtime.wasm"));

// A separate target directory, so the two builds don't overwrite each other.
const hooks = build(join(root, "target", "runtime-test-hooks"), ["--features", "test-hooks"]);
mkdirSync(join(here, "..", "test", "build"), { recursive: true });
copyFileSync(hooks, join(here, "..", "test", "build", "mesh-runtime-test.wasm"));
