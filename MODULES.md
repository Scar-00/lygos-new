# File modules

```lygos
import print;
import "helpers/math.ly" as math;

fn main() -> i32 {
    print::println$("answer = {}", math::answer());
    return 0;
}
```

Named imports search relative to the importing file: `name.ly`, then
`name/mod.ly`. Quoted paths are also relative to that file. `as` sets the module
alias; otherwise the name or quoted path's file stem becomes the alias.

Use `module::function()`, `module::Type`, `module::Type${ ... }`, and
`module::Type::method()`. Imported namespaces can also be traversed through
another module's imports. Declarations currently have public visibility.
Canonical file paths are cached so a diamond import loads each file once.
Cycles, conflicting aliases, missing files and unknown exports produce source
diagnostics before code generation.

`import print;` falls back to the bundled module if no local print module exists.
It provides `print::print$`, `print::println$`, and `print::format_args$` using
the formatting code from `tests/simple_program.ly`. An empty `println$()` emits
a newline; one argument prints text; additional arguments fill `{}` placeholders.
Booleans, string references, pointers and signed/unsigned integers are supported.

Token-arm macros can be imported and expanded in statements or expressions;
expression expansion must produce one expression. Their helper declarations
resolve in the definition file, and substituted arguments resolve in the caller.
Expansion is bounded. These macros do not provide full identifier hygiene.
Compile-time `macro fn` and attributes still execute locally within each file;
exported compile-time functions are not yet callable as namespaced macros.
Package discovery, selective imports and private declarations are not implemented.
