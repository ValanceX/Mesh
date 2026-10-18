// Puts the vectors where the tarball takes them from: `conformance/` is a
// copy of `examples/conformance/`, made when the package is packed and never
// committed (the repository's one copy is `examples/conformance/`, which the
// Rust tests read in place).
import { cpSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const from = join(here, "..", "..", "..", "examples", "conformance");
const to = join(here, "..", "conformance");

rmSync(to, { recursive: true, force: true });
cpSync(from, to, { recursive: true });
