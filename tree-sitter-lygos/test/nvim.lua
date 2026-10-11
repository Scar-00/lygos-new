-- Run with the configured Neovim: nvim -n --headless -c "luafile test/nvim.lua"
local function check()
  local script = debug.getinfo(1, 'S').source:sub(2)
  local grammar = vim.fs.dirname(vim.fs.dirname(vim.fs.normalize(vim.fn.fnamemodify(script, ':p'))))
  local sample = vim.fs.joinpath(vim.fs.dirname(grammar), 'tests', 'simple_program.ly')
  vim.cmd.edit(vim.fn.fnameescape(sample))
  local buf = vim.api.nvim_get_current_buf()
  assert(vim.bo[buf].filetype == 'lygos', '.ly filetype detection failed')
  assert(vim.treesitter.highlighter.active[buf], 'Lygos highlighter did not start')
  local parser = vim.treesitter.get_parser(buf, 'lygos')
  local root = parser:parse()[1]:root()
  assert(not root:has_error(), 'sample has syntax errors in the Tree-sitter grammar')
  -- The sample is actively edited. Use a stable scratch fixture for captures.
  vim.cmd.enew()
  buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, {
    '// Highlight fixture',
    'import print; import "helper.ly" as helper;',
    'struct S { value: i32; }',
    'impl S { fn read(&self) -> i32 { return self.value; } }',
    'fn identity(n: i32) -> i32 { return n; }',
    'macro fn Count(item: Ast) { ast::field_count(item); }',
    '#[derive(Count)] struct Tagged { value: i32; }',
    'fn run() { let s = S${1}; let texts = ["a"; 2]; let p: *const i8; identity(s.read()); value$(1); }',
    'fn module_use(value: helper::Type) { let item = helper::Type${}; print::println$("{}", value); }',
  })
  vim.bo[buf].filetype = 'lygos'
  parser = vim.treesitter.get_parser(buf, 'lygos')
  root = parser:parse()[1]:root()
  assert(not root:has_error(), 'highlight fixture has syntax errors')
  local captures = {}
  for _, kind in ipairs({ 'highlights', 'folds', 'indents' }) do
    local query = assert(vim.treesitter.query.get('lygos', kind), kind .. ' query missing')
    local count = 0
    for id in query:iter_captures(root, buf) do
      captures[query.captures[id]] = true
      count = count + 1
    end
    assert(count > 0, kind .. ' query produced no captures')
  end
  for _, capture in ipairs({ 'function', 'function.call', 'function.method.call', 'function.macro',
    'variable.member', 'variable.parameter', 'type', 'type.builtin', 'keyword.modifier',
    'string', 'comment', 'fold', 'indent.begin', 'attribute' }) do
    assert(captures[capture], 'missing capture: ' .. capture)
  end
  assert(vim.bo[buf].commentstring == '// %s', 'commentstring missing')
  assert(vim.bo[buf].indentexpr:find('nvim%-treesitter'), 'Tree-sitter indentation missing')
  assert(vim.wo.foldexpr:find('treesitter'), 'Tree-sitter folding missing')

  -- Scratch buffers exercise incremental parsing and error recovery without
  -- changing the user's source file.
  vim.cmd.enew()
  buf = vim.api.nvim_get_current_buf()
  vim.api.nvim_buf_set_lines(buf, 0, -1, false, {
    '/* outer /* nested */ outer */',
    'fn first(p: *const *mut i8) { p; let values = [[0; 4]; 2]; }',
    'fn second() -> i32 { return 1; }',
    'macro fn Count(item: Ast) { ast::field_count(item); }',
    '#[derive(Count)] struct S { value: i32; }',
  })
  vim.bo[buf].filetype = 'lygos'
  parser = vim.treesitter.get_parser(buf, 'lygos')
  assert(not parser:parse()[1]:root():has_error(), 'valid nested pointers/comments rejected')
  vim.api.nvim_buf_set_lines(buf, 1, 2, false, { 'fn first(p: *i8) { p; }' })
  root = parser:parse()[1]:root()
  assert(root:has_error(), 'bare pointer type was accepted')
  local recovered = false
  for child in root:iter_children() do
    if child:type() == 'function_declaration' then
      local name = child:field('name')[1]
      if name and vim.treesitter.get_node_text(name, buf) == 'second' then recovered = true end
    end
  end
  assert(recovered, 'syntax error swallowed the following function')
  vim.api.nvim_buf_set_lines(buf, 1, 2, false, { 'fn first(p: *mut i8) { p[0] = \'a\'; }' })
  assert(not parser:parse()[1]:root():has_error(), 'incremental repair failed')
  print('Lygos Neovim checks passed: filetype, sample, queries, highlighting, folds, indentation, incremental recovery')
end

local ok, err = pcall(check)
if not ok then
  vim.api.nvim_err_writeln(tostring(err))
  vim.cmd('cquit 1')
else
  vim.cmd('qa!')
end
