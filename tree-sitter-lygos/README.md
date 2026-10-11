# Tree-sitter for Lygos

This grammar follows the compiler's current syntax. It includes functions and
receivers, structs, enums, traits and impls, aliases, statics, nested pointer and
reference types, arrays and slices, function types, control flow, casts such as
`(:i64)value`, `Type${ ... }` initializers, and `$` macro token trees. Expression
precedence matches the compiler. Raw pointer types require `*const` or `*mut`.
The external scanner handles nested block comments.
Executable `macro fn` declarations, `#[derive(...)]` and declaration attributes
are supported, as are repeat array initializers `[value; length]`.
File imports, aliases, qualified types and namespaced macro calls are supported.

Tree-sitter recognizes syntax and recovers while editing. It does not check
types or implement macro expansion. Keywords for unimplemented compiler features
such as `while`, `match`, and generics have not been added as language constructs.

The generated C parser and node metadata are included. ABI 14 works with the
installed Neovim's supported range. The project pins Tree-sitter CLI 0.27.1
locally, so it does not require replacing an existing global CLI installation.

## Develop and test

From this directory:

```powershell
npm ci
npm run generate
npm test
npx tree-sitter parse --quiet ../tests/simple_program.ly
```

Generation uses Node.js; parser compilation needs a C compiler. The CLI can use
the installed MSVC toolchain on Windows. Eight corpus tests cover declarations,
types, precedence, postfix chains, control flow, macros, Unicode, and comments.
Review expected trees when changing the grammar; use `npx tree-sitter test -u`
only when intentionally updating those expectations.

## Neovim

`nvim/lygos.lua` integrates with the current `nvim-treesitter` API. Copy it to
your config's `lua/lygos.lua` and add this after Tree-sitter setup, using the
absolute path to this grammar directory:

```lua
require('nvim-treesitter').setup()
require('lygos').setup({
  parser_path = 'C:/dev/learning/lygos/tree-sitter-lygos',
})
```

Then run `:TSInstall lygos`. The module registers the local grammar, enables `.ly`
file detection, starts highlighting, configures indentation and folding, and
sets `//` as the comment prefix. Folds start open. Highlight, indent, and fold
queries live in `queries/`; the installer links these into Neovim's runtime.
The pinned CLI is added to this Neovim process's PATH so the installer can use
the required modern CLI commands.

This is already installed in `C:/Users/fiffi/AppData/Local/nvim` and its
`nvim-data/site` runtime. Restart an existing Neovim session to load the config.
After editing the grammar, run `npm run generate`, then `:TSUpdate lygos` to
recompile it. Changes to linked queries are picked up on the next Neovim session.
If the checkout moves, update `parser_path` and reinstall the parser.

With the configured Neovim, run `npm run test:nvim` to verify file detection,
parsing of the sample, highlight captures, folds, indentation, and incremental
error recovery. Scratch buffers used for the test do not write to the sample.

References: [Tree-sitter grammar DSL](https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html),
[parser generation](https://tree-sitter.github.io/tree-sitter/cli/generate.html),
and the installed `nvim-treesitter` README's custom parser instructions.
