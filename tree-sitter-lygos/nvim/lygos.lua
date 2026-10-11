local M = {}

function M.setup(opts)
  local parser_path = vim.fs.normalize(assert(opts.parser_path, 'Lygos parser_path is required'))
  local group = vim.api.nvim_create_augroup('LygosTreesitter', { clear = true })

  -- Use the pinned project CLI for this Neovim process. This also keeps
  -- :TSInstall/:TSUpdate compatible with the current nvim-treesitter API.
  local cli_dir = vim.fs.joinpath(parser_path, 'node_modules', 'tree-sitter-cli')
  local windows = vim.fn.has('win32') == 1
  local cli = vim.fs.joinpath(cli_dir, windows and 'tree-sitter.exe' or 'tree-sitter')
  if vim.fn.executable(cli) == 1 then
    local separator = windows and ';' or ':'
    local prefix = cli_dir .. separator
    if (vim.env.PATH or ''):sub(1, #prefix) ~= prefix then
      vim.env.PATH = prefix .. (vim.env.PATH or '')
    end
  end

  local function register()
    require('nvim-treesitter.parsers').lygos = {
      install_info = { path = parser_path, queries = 'queries', generate = false },
      tier = 2,
    }
  end
  register()
  vim.api.nvim_create_autocmd('User', { group = group, pattern = 'TSUpdate', callback = register })
  vim.filetype.add({ extension = { ly = 'lygos' } })
  vim.treesitter.language.register('lygos', 'lygos')

  vim.api.nvim_create_autocmd('FileType', {
    group = group,
    pattern = 'lygos',
    callback = function(event)
      vim.bo[event.buf].commentstring = '// %s'
      local ok, err = pcall(vim.treesitter.start, event.buf, 'lygos')
      if not ok then
        vim.notify('Lygos Tree-sitter: ' .. tostring(err) .. '\nRun :TSInstall lygos.', vim.log.levels.WARN)
        return
      end
      vim.bo[event.buf].indentexpr = "v:lua.require'nvim-treesitter'.indentexpr()"
      vim.wo.foldmethod = 'expr'
      vim.wo.foldexpr = 'v:lua.vim.treesitter.foldexpr()'
      vim.wo.foldlevel = 99
    end,
  })
end

return M
