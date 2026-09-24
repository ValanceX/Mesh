-- MESH in Neovim 0.11 or later: the mesh-lsp language server, and MPRX
-- highlighting from the grammar's Tree-sitter queries.
-- See docs/guides/editor-setup.md.

-- `.mprx` files are MPRX. Neovim sends the filetype as the `languageId`,
-- and mesh-lsp checks only documents whose `languageId` is `mprx`.
vim.filetype.add({ extension = { mprx = "mprx" } })

vim.lsp.config("mesh", {
  cmd = { "mesh-lsp" },
  -- JSON too, so the manifest gets its diagnostics and its unsaved edits
  -- are seen. Other JSON files are ignored by the server.
  filetypes = { "mprx", "json" },
  root_markers = { "components.json", ".git" },
  -- The `"mesh"` settings: the manifest, and each file's component.
  init_options = {
    model = "components.json",
    components = {
      ["users-page.mprx"] = "users-page",
      ["user-card.mprx"] = "user-card-example",
    },
  },
})
vim.lsp.enable("mesh")

-- Highlighting, once the parser and query are installed (see the guide).
vim.api.nvim_create_autocmd("FileType", {
  pattern = "mprx",
  callback = function(args)
    pcall(vim.treesitter.start, args.buf, "mprx")
  end,
})
