/**
 * The five shipped targets (outline v0.4 D8): `${process.platform}-${process.arch}`
 * to the platform package holding that target's binary, and the binary's
 * file name in its `bin/`. `crates/mesh-cli/tests/packages.rs` checks that
 * these are exactly `@valancex/mesh-lsp`'s optional dependencies.
 */
export interface Target {
  package: string;
  binary: string;
}

export const TARGETS: Record<string, Target> = {
  "darwin-arm64": { package: "@valancex/mesh-lsp-darwin-arm64", binary: "mesh-lsp" },
  "darwin-x64": { package: "@valancex/mesh-lsp-darwin-x64", binary: "mesh-lsp" },
  "linux-arm64": { package: "@valancex/mesh-lsp-linux-arm64", binary: "mesh-lsp" },
  "linux-x64": { package: "@valancex/mesh-lsp-linux-x64", binary: "mesh-lsp" },
  "win32-x64": { package: "@valancex/mesh-lsp-win32-x64", binary: "mesh-lsp.exe" },
};
