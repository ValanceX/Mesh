# MPRX for VS Code

The MPRX language for VS Code: a TextMate grammar (`syntaxes/mprx.tmLanguage.json`) and bracket and quote rules. Nothing else.

This directory is where the grammar lives. The Tree-sitter grammar in `../../grammar/tree-sitter-mprx` colours MPRX for Neovim and Helix; VS Code (and GitHub's highlighter) read TextMate grammars, so MPRX has both, and both are tested against the same files in this repository.

Diagnostics, quick fixes, hover, go to definition and completion are `mesh-lsp`'s, in any editor ([editor setup](../../docs/guides/editor-setup.md)). A framework's extension (VALANCE's) starts that server with the settings its project generates and depends on this one for the language.

```console
$ npm ci && npm test      # the grammar, with VS Code's own engine, on this repository's MPRX
```

Not published. A publisher account is deferred until the VS Code integration is proven (see VALANCE's `docs/application-model/EDITOR-FEASIBILITY.md`).
