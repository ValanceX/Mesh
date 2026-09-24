-- Checks the editor setup guide's configuration in a real Neovim: run by
-- verify.sh, with init.lua loaded and the current directory a copy of
-- examples/. Exits 0 if every check passes, 1 at the first failure.

local failures = 0
local function check(ok, what, detail)
  if ok then
    io.stdout:write("ok   " .. what .. "\n")
  else
    failures = failures + 1
    io.stdout:write("FAIL " .. what .. ": " .. vim.inspect(detail) .. "\n")
  end
end

local function finish()
  io.stdout:write(failures == 0 and "all checks passed\n" or (failures .. " failed\n"))
  vim.cmd(failures == 0 and "qall!" or "cquit 1")
end

-- Every publication, by URI, so a check can wait for the server's word.
local published = {}
local publish = vim.lsp.handlers["textDocument/publishDiagnostics"]
vim.lsp.handlers["textDocument/publishDiagnostics"] = function(err, result, ctx, config)
  published[result.uri] = (published[result.uri] or 0) + 1
  return publish(err, result, ctx, config)
end

local function wait_for_publication(buf, after)
  local uri = vim.uri_from_bufnr(buf)
  return vim.wait(5000, function()
    return (published[uri] or 0) > after
  end, 20)
end

local function request(buf, method, line, character)
  local params = {
    textDocument = { uri = vim.uri_from_bufnr(buf) },
    position = { line = line, character = character },
  }
  local responses = vim.lsp.buf_request_sync(buf, method, params, 5000) or {}
  for _, response in pairs(responses) do
    return response.result, response.err
  end
end

local function labels(result)
  local items = result and (result.items or result) or {}
  return vim.tbl_map(function(item)
    return item.label
  end, items)
end

local function codes(buf)
  return vim.tbl_map(function(diagnostic)
    return diagnostic.code
  end, vim.diagnostic.get(buf))
end

-- Replaces line `line` (0-based) and waits for the server to check it.
local function set_line(buf, line, text)
  local uri = vim.uri_from_bufnr(buf)
  local before = published[uri] or 0
  vim.api.nvim_buf_set_lines(buf, line, line + 1, false, { text })
  return wait_for_publication(buf, before)
end

-- The page ------------------------------------------------------------------

vim.cmd("edit users-page.mprx")
local page = vim.api.nvim_get_current_buf()
check(vim.bo[page].filetype == "mprx", "users-page.mprx is filetype mprx", vim.bo[page].filetype)
local attached = vim.wait(5000, function()
  return #vim.lsp.get_clients({ bufnr = page, name = "mesh" }) > 0
end, 20)
check(attached, "mesh-lsp attaches to users-page.mprx")
if not attached then
  return finish()
end
check(wait_for_publication(page, 0), "diagnostics are published")
check(#vim.diagnostic.get(page) == 0, "users-page.mprx has no diagnostics", codes(page))

local hover = request(page, "textDocument/hover", 1, 15) -- user.na|me
check(hover and hover.contents.value:find("string", 1, true), "hover on user.name is string", hover)
hover = request(page, "textDocument/hover", 2, 52) -- comp|act
check(hover and hover.contents.value:find("boolean", 1, true), "hover on compact is boolean", hover)

local definition = request(page, "textDocument/definition", 2, 4) -- av|atar
local location = definition and (definition[1] or definition)
check(
  location
    and location.uri == vim.uri_from_fname(vim.fn.getcwd() .. "/components.json")
    and location.range.start.line == 34,
  "definition of avatar is its key in components.json",
  location
)

-- Highlighting --------------------------------------------------------------

local parsed = pcall(function()
  vim.treesitter.get_parser(page, "mprx"):parse(true)
end)
check(parsed, "the MPRX parser is installed")
local function captures(line, column)
  return vim.tbl_map(function(capture)
    return capture.capture
  end, vim.treesitter.get_captures_at_pos(page, line, column))
end
for _, expected in ipairs({
  { 0, 2, "tag" },
  { 0, 7, "attribute" },
  { 0, 14, "string" },
  { 1, 10, "variable" },
  { 1, 14, "variable.member" },
  { 2, 52, "variable" },
  { 3, 47, "function.call" },
  { 3, 58, "variable.builtin" },
}) do
  local found = captures(expected[1], expected[2])
  check(vim.tbl_contains(found, expected[3]), ("capture %s at %d:%d"):format(expected[3], expected[1], expected[2]), found)
end

-- Completion on broken text -------------------------------------------------

local original = vim.api.nvim_buf_get_lines(page, 1, 2, false)[1]
check(set_line(page, 1, "  <avatar "), "a half-typed tag is checked")
local completion = request(page, "textDocument/completion", 1, 10)
check(vim.deep_equal(labels(completion), { "alt", "src", "size" }), "completion in <avatar offers alt, src, size", labels(completion))

check(set_line(page, 1, "  {user.}"), "{user.} is checked")
completion = request(page, "textDocument/completion", 1, 8)
check(vim.deep_equal(labels(completion), { "active", "avatar", "name" }), "completion after user. offers its fields", labels(completion))
local only_syntax = #vim.diagnostic.get(page) > 0
for _, code in ipairs(codes(page)) do
  only_syntax = only_syntax and code == "syntax-error"
end
check(only_syntax, "only syntax errors are published for {user.}", codes(page))

check(set_line(page, 1, original), "the page is restored")
check(#vim.diagnostic.get(page) == 0, "the restored page has no diagnostics", codes(page))

-- The manifest --------------------------------------------------------------

vim.cmd("edit components.json")
local manifest = vim.api.nvim_get_current_buf()
check(
  vim.wait(5000, function()
    return #vim.lsp.get_clients({ bufnr = manifest, name = "mesh" }) > 0
  end, 20),
  "mesh-lsp attaches to components.json"
)
-- An unsaved edit to the manifest re-checks the page: renaming the
-- users-page scope name `user` breaks every reference to it.
local lines = vim.api.nvim_buf_get_lines(manifest, 0, -1, false)
for index, text in ipairs(lines) do
  local renamed = text:gsub('"user": { "kind": "named"', '"person": { "kind": "named"')
  if renamed ~= text then
    vim.api.nvim_buf_set_lines(manifest, index - 1, index, false, { renamed })
    break
  end
end
local rechecked = vim.wait(5000, function()
  return vim.tbl_contains(codes(page), "unknown-reference")
end, 20)
check(rechecked, "editing the manifest re-checks the page against the edit", codes(page))

finish()
