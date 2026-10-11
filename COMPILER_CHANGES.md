# Compiler changes

## Typed intermediate representation and context arenas

The active compilation pipeline is now:

```text
source -> lexer -> parser AST -> compile-time expansion -> semantic analysis -> TypedModule -> LLVM
```

`TypeChecker::analyze` validates the source and builds an owned `TypedModule`.
`CodeGenerator::generate` accepts only that module. The parsed AST can be dropped
before generating LLVM; execution tests do exactly this. The AST crate no longer
depends on LLVM, and source nodes no longer contain LLVM values or scope pointers.

This separation was needed because the old visitors mixed name resolution,
type checking and LLVM emission. The backend could disagree with the checker,
repeat lookups, or encounter missing semantic information halfway through emitting
a function. The new representation records resolved symbols, canonical types,
field indices, call signatures, receiver adjustments, loads and conversions.
LLVM lowering consumes those decisions rather than reconstructing them.

`SemanticContext` owns append-only arenas for expressions, statements, blocks,
functions, symbols and scopes, plus an interner for types. Edges use distinct
`ExprId`, `StmtId`, `BlockId`, `FunctionId`, `SymbolId`, `ScopeId` and `TypeId`
integer types. Growing a vector can move its storage, but does not change an ID's
index. Rust borrows prevent retaining an element reference while mutating its
arena. Scope parents use IDs, replacing the former raw-pointer scope graph.
Shadowed bindings get separate symbols; aliases share canonical type IDs.

IDs belong to their originating context. They are not globally unique, and an ID
from another module must not be used here. Entries are never deleted or reused,
so generation counters are unnecessary for this compilation model. Dropping the
module releases all its arenas together. The public module API provides read-only
access after construction; LLVM caches remain separate from semantic storage.

The source AST remains an owned syntax tree for parsing and diagnostics. During
analysis, a temporary map associates source-node identities with checked type
facts; those identities are never dereferenced and do not survive into the typed
module. Trait declarations are validated before runtime lowering; this rework
does not introduce runtime trait dispatch. Executable `macro fn` definitions and
attributes now expand before this pass; see [COMPTIME_MACROS.md](COMPTIME_MACROS.md).

The old AST visitor backend, symbol collector, preprocessor and raw-pointer scope
APIs have been removed. Compiler integrations should use `TypeChecker::analyze`
followed by `CodeGenerator::generate`. To inspect the intermediate representation:

```powershell
cargo run -- path/to/program.ly --emit-type typed-ast
```

Regression tests cover stable arena IDs, distinct ID kinds, canonical aliases,
recursive nominal types, shadowing, explicit casts, resolved calls and receivers,
and LLVM execution after dropping the source AST. Formatting runtime contracts
are checked during analysis. Static strings support embedded NUL bytes. A
`sizeof$` regression also exposed an LLVM wrapper that assigned into unconstructed
`DataLayout` storage; it now borrows the module's initialized layout and uses
allocation size, including padding.

## Parser and type-system fixes

The frontend now checks programs before generating LLVM IR. The changes below
address reproducible parser, type-checking and lowering failures.

## Why the larger changes were needed

### Expression parsing and parser state

Assignment previously bound more tightly than multiplication, and unary and cast
operands parsed an entire expression. For example, `*p + x` became `*(p + x)`.
The parser now has explicit precedence levels and right-associative assignment.
Calls, fields, indices and `::` share one postfix loop, allowing expressions such
as `a[0].field.method()[1]` and `factory()()`.

The parser also kept raw pointers to local blocks. Nested control flow could
leave a dangling pointer that a subsequent `return` wrote through. Parser context
now stores owned impl/trait metadata, and return parsing does not mutate another
block through a pointer. EOF handling is bounded, including empty token streams.

### Semantic validation before LLVM

Previously, type checks were scattered through code generation. Missing checks
could reach `todo!()`, silently accept incompatible values, or emit invalid IR.
`lygos-ast::type_check::TypeChecker` is an independent pass with source locations.
Its `check` API returns the first error; `check_all` collects errors from
independent function bodies. It validates:

- Forward declarations, alias expansion, alias cycles and recursive struct layout.
- Function signatures, all call arguments, returns and required return paths.
- Struct initializers, fields, index types and literal array bounds.
- Branch/loop scopes, conditions, mutation and definite initialization.
- Trait method signatures, including substitution of `Self`.

Types compare by structure, independently of source locations. References retain
their mutability distinction; aliases are expanded for semantic comparisons.
Every raw pointer must explicitly use `*const T` or `*mut T`, including nested
pointer types. Bare `*T` is rejected with a diagnostic showing both spellings.
`*const T` prevents writes through the pointer; `*mut T` permits them, including
`self.data[index] = char`. The pointer binding can be immutable while the
pointed-to memory is writable. `let mut` controls replacing the binding and
does not make a const pointer writable. References retain `&T` and `&mut T`.

Numeric conversions on annotated variable declarations remain supported. Other
conversions require explicit casts. Reference values require explicit `*` when a
value is needed; member access retains automatic access through a reference.

### Consistent lowering of addresses, values and calls

Code generation now distinguishes a storage address from a loaded expression
value. This fixes dereferencing fields, pointer indexing, references, computed
method receivers and chained calls. Callee signatures are resolved independently
of return types: a function returning a function pointer remains a direct call.
Indirect calls also check arity, and member calls check every explicit argument.

Returns now emit terminators at the return statement. Branches and loops stop
emitting instructions after a terminator, and use their own lexical scopes.
Logical operators short-circuit. LLVM operations and casts select floating-point,
signed or unsigned behavior from the source-language type.

### Lexer and command-line behavior

The lexer handles missing sentinels, comments at EOF, adjacent/nested block
comments, escaped quotes, leading underscores and operator spans. Its fallible
`try_get_tokens` API reports malformed input; the CLI uses that API. Literal
escapes are decoded consistently, including embedded NUL bytes in strings.
Formatting placeholders are validated before lowering.

Diagnostics highlight the complete failing expression and relevant declarations.
Pointer writes describe the actual read-only pointer type and show the required
`*mut` spelling; mutable method calls have their own diagnostic. Argument errors
name the callee and argument number. Delimiter errors identify both the opener
and offending closer, or EOF. Independent function errors are reported together,
with function names and an error count. Captured output contains no ANSI escapes,
and unavailable source files fall back to text rather than causing a panic.

The executable now compiles the supplied input file. Previously it ignored CLI
input and always JIT-called `add` in `tests/simple_program.ly`. It supports:

```powershell
cargo run -- path/to/program.ly --check
cargo run -- path/to/program.ly -e ast
cargo run -- path/to/program.ly -e llvm-ir -o program.ll
cargo run -- path/to/program.ly -o program.exe
cargo test --workspace
```

On Windows, executable linking uses Cargo's `cc` toolchain discovery to supply
Visual C++ and Windows SDK library paths to the Clang child process. Previously
an ordinary shell could fail with `libcmt.lib` missing even though Cargo built the
compiler successfully. No developer shell or global environment change is now
needed. A CLI regression compiles and runs a program importing the print module
and checks its actual output. Checking and emitting IR do not invoke the linker.

File modules are resolved before type checking and code generation, with cached
file IDs and canonical declaration names. This lets the existing typed pipeline
handle namespaces without duplicating a file's statics or implementations when
several modules import it. See [MODULES.md](MODULES.md) for syntax and limits.
The bundled print module adapts the formatter and token macros from
`tests/simple_program.ly`; string descriptor access and borrowing were added so
formatting pieces remain reference-only. Its Windows adapter binds the CRT's
`fwrite`, while POSIX uses `write`.

## Compatibility and remaining feature limits

Compile-time meta definitions now live in [meta/builtins.ly](meta/builtins.ly),
covering `Ast`, `AstExpr`, `AstDecl`, `AstField` and `AstType`. Their `#[meta]`
declarations are validated and erased. Fields and source annotations have typed
handles and methods for inspection and mutation; these handles do not become
runtime values. See [COMPTIME_MACROS.md](COMPTIME_MACROS.md) for the API.

Arrays support repeat initializers: `[value; length]`, for example `[0; 16]`,
`["hello"; 3]` or `[[0; 4]; 2]`. The length must be a non-negative integer
literal. The value is evaluated once and copied into each element, including
aggregate values; references retain their shared referent. A zero-length repeat
still evaluates its initializer and retains its element type. LLVM uses a loop
instead of expanding the source or emitting one store per element. Existing
`[a, b, c]` literals remain supported. Tree-sitter and the installed Neovim parser
recognize the new syntax.

`str` is now reference-only. String literals have type `&str`, so inferred
bindings hold references. Bare `str` variables, fields, parameters, return types,
array elements and function-pointer signatures are rejected, including aliases.
Dereferencing or indexing a string pointer to obtain a `str` value is rejected,
as is `sizeof$(str)`. Use `&str` in string APIs. Raw pointers to string storage
remain supported, but cannot be loaded as bare string values.

Literal bytes and their pointer/length descriptors live in immutable globals;
returning a literal reference does not leave a pointer into a function's stack.
This preserves the existing descriptor layout behind the reference. Formatting
borrows that storage directly rather than formatting the reference's address.
This does not add general unsized-type or borrow checking support. Existing
sample APIs taking or returning bare `str` need to migrate to reference APIs.

Programs that modify a `let` binding need `let mut`; mutable reference receivers
need `&mut self`. Trait implementations must match the trait signature. These
checks may expose errors in examples that previously passed unchecked.
Existing raw-pointer annotations and casts must be migrated to explicit
`*const` or `*mut`; the sample and regression programs have been updated.

Executable `macro fn` definitions support inline calls, attributes and derives
through LLVM ORC LLJIT execution and a checked host AST API. The older token-arm macro
declarations still parse but their expansion remains unsupported. `sizeof$`
currently accepts a named type. Generics, default
trait bodies, nested namespaces and reference lifetime/borrow analysis remain
outside the implemented language. Integer literals retain their `i32` default;
empty array literals still need further contextual-inference support.

Regression coverage includes parser AST shapes, accepted/rejected semantic
programs, CLI diagnostics and LLVM verification plus execution through the JIT.
The workspace regression suite passes. The evolving `simple_program.ly` parses, but its
semantic check still reports errors such as immutable loop counters, extra
reference levels and read-only formatter receivers. These examples require
source changes to meet the checked language rules.


## LLVM compile-time execution

LLVM lowering now lives in `lygos-codegen`, shared by the CLI and macro expander.
This removes the dependency cycle that would otherwise arise if the parser
imported the application's backend. `lygos::code_gen` still re-exports the same
public generator API.

Macro control flow lowers to an opaque value-handle ABI, is checked into the
existing typed IR, and executes as LLVM-generated functions through ORC LLJIT.
The handwritten recursive expression/statement interpreter has been removed.
Checked value operations and AST methods remain host callbacks, preserving the
current macro semantics and diagnostics while using LLVM for calls, branches,
loops and local bindings. This first migration does not yet compile arbitrary
runtime functions at compile time or lower all value operations to native LLVM
instructions. See `COMPTIME_MACROS.md` for the supported boundary.

The JIT wrapper now supports explicit host-symbol registration, propagates LLVM
errors, and releases engines on drop. Module transfer copies bitcode into an
ORC-owned context: the old wrapper paired a module with a different context and
could leave it referencing a destroyed caller context. LLVM target initialization
is guarded by `Once` for concurrent expansion. Native callbacks receive an
explicit invocation context and cannot unwind through machine-code frames.

Fixed-size local allocations now go in the function entry block. Previously,
loop-local bindings and temporary aggregates emitted `alloca` inside the loop,
so each iteration consumed more stack space. Native macro instrumentation made
this visible as a stack overflow before the execution budget could report an
error. The fix applies to runtime functions too; regression coverage includes a
30,000-iteration loop with a local array and runaway native macro loops.
