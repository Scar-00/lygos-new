# Compile-time macros and declaration attributes

Compile-time functions execute during expansion, before runtime type checking.
They use `macro fn`; calls retain `name$(args)`. These definitions are removed
from the runtime `TypedModule`, so no macro execution code is emitted into the
program.

[examples/comptime.ly](examples/comptime.ly) combines all three entry points.
Run `cargo run -- examples/comptime.ly --check` or inspect expansion with
`cargo run -- examples/comptime.ly -e ast`. With a correctly typed initializer, the compiled example returns 20.
The added `tag` initializer must match the type chosen by `add_tag`; for `&str`,
use a string literal.

```lygos
macro fn square(value: i32) -> i32 {
    return value * value;
}

fn example() -> i32 {
    return square$(7); // expands to 49
}
```

Value parameters require compile-time expressions. Runtime variables cannot be
passed as compile-time integers. Use an `Ast` parameter when a macro should
receive unevaluated runtime syntax:

```lygos
macro fn twice(value: Ast) -> Ast {
    return value.binary("+", value.clone());
}

fn example(value: i32) -> i32 {
    return twice$(value);
}
```

`twice$(f())` expands to two calls to `f()`: copying syntax does not evaluate or
cache an expression. Identifiers in generated syntax resolve in the expansion's
runtime scope. Quoted syntax is not automatically hygienic; choose generated
names deliberately. `ast::parse_expr` parses a string into one expression;
`ast::parse_item` parses one declaration. Structured constructors are also
available, so transformations do not have to construct source strings.

## Attributes and derives

An attribute receives its declaration as the first `Ast` argument. Further
arguments are compile-time values. Attributes run in source order. Returning
`void` keeps the declaration, including mutations made through its handle;
returning `Ast` replaces it with that declaration.

```lygos
macro fn named(item: Ast, name: &str) {
    item.rename(name);
}

macro fn add_tag(item: Ast) {
    item.add_field("tag", "i32");
}

#[named("Tagged")]
#[add_tag]
struct Original { value: i32; }
```

`#[derive(Name, Other)]` invokes each named compile-time function with the
declaration handle. Derive functions return `void` and use `ast::emit` to add
module declarations. Derives are user-defined functions; names such as `Debug`
do not imply a compiler-provided implementation.

```lygos
macro fn FieldCount(item: Ast) {
    let name = item.name();
    let count = item.field_count();
    ast::parse_item(
        "impl " + name + " { fn field_count(&self) -> i32 { return "
        + ast::to_string(count) + "; } }"
    ).emit();
}

#[derive(FieldCount)]
struct Pair { x: i32; y: i32; }

fn example() -> i32 {
    let pair = Pair${1, 2};
    return pair.field_count(); // 2
}
```

Generated declarations are expanded too, then checked alongside the original
program. Duplicate fields, incompatible expressions, invalid signatures and
missing names therefore produce ordinary compiler errors. Generated source
locations point to the invocation; execution errors also show the compile-time
function declaration. Attributes currently apply to module declarations.

## AST operations

The compiler-provided definitions and signatures live in
[meta/builtins.ly](meta/builtins.ly). `#[meta]` marks opaque compile-time type
and method declarations. The compiler validates these against its embedded
catalog and erases them before runtime analysis; the empty structs do not
define a runtime representation. The types are available to macros without an
import. The catalog itself can be parsed and checked as Lygos source.

The specialized types are:

| Type | Represents |
| --- | --- |
| `Ast` | A general syntax node |
| `AstExpr` | An expression node |
| `AstDecl` | A declaration node |
| `AstField` | A live field in a struct declaration |
| `AstType` | A snapshot of a source type annotation |

Expression and declaration handles support the general node operations.
`node.as_expr()` and `node.as_decl()` validate a node's kind. Specialized parameter,
local and return annotations reject incompatible handles. For example:

```lygos
macro fn sum(left: AstExpr, right: AstExpr) -> AstExpr {
    return left.binary("+", right);
}

macro fn change(item: AstDecl) {
    let fields: [AstField] = item.fields();
    let field: AstField = fields[0];
    field.rename("value");
    field.set_type(ast::parse_type("i32"));
}
```

`node.field(index)` and `node.fields()` expose field handles. Field handles
provide `name()`, `rename(name)`, `field_type()` and `set_type(type)`. They refer
to the parent declaration, so mutations change the generated runtime layout.
Replacing the parent invalidates earlier field handles, even if the replacement
has the same field count; using an invalidated handle produces a diagnostic.

`ast::parse_type(source)` creates an `AstType`. Type handles provide `name()`,
`kind()`, `clone()`, `is_reference()`, `is_mutable()`, `element()` and
`array_len()`. `element()` applies to pointers, references, arrays and slices.
These are source annotations, not inferred semantic types or LLVM types. Aliases
retain their source spelling. Type snapshots are independent of later field
mutations. Meta arrays support `len()` during compile-time execution.

`Ast` is a context-local typed integer handle into expansion-owned syntax storage.
Assigning a handle shares its node; `ast::clone` creates an independent node.
Handles cannot be used in runtime code or retained by the LLVM backend.

Node operations use methods: `node.name()`, `node.kind()`, `node.clone()`,
`node.rename(name)`, `node.replace(replacement)`, `node.emit()`,
`node.field_count()`, `node.field_name(index)`, `node.field_type(index)` and
`node.add_field(name, type)`. `left.binary(operator, right)` creates an expression
using the receiver as its left operand. Method receivers are evaluated once;
computed receivers and chaining such as `ast::parse_item(source).clone().emit()`
are supported. Mutations affect the shared handle; cloning creates independent
syntax. Constructors such as `ast::parse_item` remain namespace operations.
The existing free functions below remain available with identical behavior.

| Operation | Behavior |
| --- | --- |
| `ast::parse_expr(source)` | Parse one expression and return its handle |
| `ast::parse_item(source)` | Parse one declaration and return its handle |
| `ast::int(value)` | Construct an integer expression |
| `ast::ident(name)` | Construct an identifier expression |
| `ast::binary(operator, left, right)` | Construct a binary expression from two handles |
| `ast::clone(node)` | Copy syntax into a new handle |
| `ast::replace(node, replacement)` | Replace the node referenced by a handle |
| `ast::name(node)` | Read a declaration or identifier name |
| `ast::rename(node, name)` | Change that name |
| `ast::kind(node)` | Return `struct`, `function`, `enum`, `alias`, `identifier` or `expression` |
| `ast::field_count(node)` | Count struct fields |
| `ast::field_name(node, index)` | Read a struct field name |
| `ast::field_type(node, index)` | Read a field's type spelling |
| `ast::add_field(node, name, type)` | Append a struct field |
| `ast::emit(node)` | Add a runtime declaration to the module |
| `ast::to_string(value)` | Convert an integer, bool or string to a string |
| `ast::error(message)` | Stop expansion with a diagnostic |

## Execution model and current boundaries

Compile-time execution supports initialized `let`/`let mut` bindings, assignment,
integer arithmetic and comparisons, strings and concatenation, bool operations,
short-circuiting, array literals/repeats/indexing, `if`, `for`, `break`, `return`,
and calls to other `macro fn` helpers. Supported annotations are `Ast`, `AstExpr`,
`AstDecl`, `AstField`, `AstType`, `&str`,
`bool`, `i32`, `i64`, `u32`, `u64`, fixed arrays of supported values, and `void`
returns, plus slices of supported values. Strings remain reference-only at the language boundary. Compile-time
integer arithmetic uses checked `i64`; unsigned annotations restrict values to
non-negative integers. Runtime literals retain their existing `i32` default.

Macro bodies lower to an internal module with an opaque value-handle ABI, then
pass through the same `TypeChecker`, `TypedModule` and `lygos-codegen` LLVM
backend as runtime programs. ORC LLJIT executes the resulting machine code.
Functions, helper calls, branches, short-circuit evaluation, loops, assignments
and returns execute as native control flow; the recursive AST interpreter has
been removed. A compiled macro module is cached for later invocations within
one expansion. Constant argument expressions use this LLVM path too.

The initial ABI represents compile-time values as context-local integer handles.
AST mutation, strings, arrays, checked integer arithmetic and value/type guards
are registered host callbacks. This keeps specialized meta handles opaque and
preserves overflow, bounds and stale-field diagnostics across the native
boundary. It does **not** yet lower every primitive value operation to a native
LLVM instruction, or make all runtime language features executable at compile
time. Source meta annotations are validated by host guards; the shared type
checker validates the lowered ABI module. ABI lowering currently constructs and
parses an internal source module before analysis.

The execution context is passed explicitly to callbacks, so concurrent compiler
invocations have independent arenas and errors. Callback errors propagate through
native guards and include the active macro call chain. Host panics are caught at
the callback boundary. On first use, macro bodies are lowered together, so invalid
names or immutable assignments in another macro body can now be diagnosed before
that body executes.

This remains a defined subset of Lygos rather than a general Zig-style evaluator
for every runtime construct. Floating-point
execution, pointer operations, arbitrary runtime/extern calls, struct values,
type reflection after semantic analysis and user-defined compile-time types are
not implemented. Unsupported operations produce diagnostics. Macro functions
must be declared at module level; forward references to them are supported.

Execution has a 100000-step budget, 16 nested function calls, and 64 nested macro
expansions. Syntax allocations have a separate node budget. Strings and quoted
source are limited to 1 MB, and repeat arrays to 10000 elements. These limits
turn runaway compile-time work into diagnostics. Native invocations also have
an 8-million-unit value-storage budget (32 units per scalar plus string bytes and
array contents); this bounds retained host temporaries. No filesystem, network
or process operations are exposed by the compile-time host API.

The older `macro name { (...) -> {...} }` syntax expands token arms through the
file module loader, including imported macros such as `print::println$`.
See [MODULES.md](MODULES.md). Use `macro fn` for executable compile-time code;
direct parser clients must use the module loader to enable token-arm expansion.

`Parser::build_ast` performs expansion automatically. Compiler integrations
needing the original syntax can use `build_raw_ast` and explicitly call
`lygos_parser::comptime::expand`, which returns expansion errors. `-e ast` shows
the expanded AST; `--emit-type typed-ast` shows the checked runtime representation.
Tree-sitter and the installed Neovim parser recognize the new definitions and
attribute syntax.
