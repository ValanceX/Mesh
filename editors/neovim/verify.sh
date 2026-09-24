#!/usr/bin/env bash
# Checks docs/guides/editor-setup.md's Neovim configuration in a real
# Neovim, headless: builds mesh-lsp and the MPRX parser, installs them
# where the guide says (inside a temporary directory, not your own
# Neovim), and runs verify.lua on a copy of examples/.
#
#   editors/neovim/verify.sh            # uses `nvim` from PATH
#   NVIM=/path/to/nvim editors/neovim/verify.sh
#   MESH_LSP=~/.cargo/bin/mesh-lsp editors/neovim/verify.sh   # an installed server
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
nvim="${NVIM:-nvim}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

"$nvim" --version | head -n 1

# mesh-lsp, on PATH as `cargo install` would put it: MESH_LSP if set (an
# installed binary), otherwise built from this checkout.
mkdir -p "$work/bin"
if [ -n "${MESH_LSP:-}" ]; then
  cp "$MESH_LSP" "$work/bin/mesh-lsp"
else
  cargo build --quiet --manifest-path "$repo/Cargo.toml" -p mesh-lsp
  cp "$repo/target/debug/mesh-lsp" "$work/bin/"
fi
export PATH="$work/bin:$PATH"

# The parser and the query, where the guide installs them, under a data
# directory of our own.
export XDG_DATA_HOME="$work/data"
site="$XDG_DATA_HOME/nvim/site"
mkdir -p "$site/parser" "$site/queries/mprx"
(cd "$repo/grammar/tree-sitter-mprx" && npx --no-install tree-sitter build -o "$site/parser/mprx.so")
cp "$repo/grammar/tree-sitter-mprx/queries/highlights.scm" "$site/queries/mprx/"

# A workspace: a copy of the examples.
cp -r "$repo/examples" "$work/workspace"
cd "$work/workspace"
git init --quiet .

XDG_CONFIG_HOME="$work/config" "$nvim" --headless -n -i NONE \
  -u "$repo/editors/neovim/init.lua" \
  -c "luafile $repo/editors/neovim/verify.lua"
