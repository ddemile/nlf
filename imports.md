## NLF module resolution

### Module A
This module exports a list of functions, let's say that it exports the function `add`.

The `CompilerBuild` contains an export field in this format `HashMap<String, Slot>`

### Module B
The `CompilerBuild` of this module contains a list of all import statements: `Vec<Import>`

For each import statement we emit a `Op::Import(ImportId)` which will get or load the wanted module before pushing all imported functions as `Value::Closure(closure_id)` to the stack.

We then store the pushed values to the *module B* locals using `Op::Store` or `Op::StoreCaptured` instructions.

This allows wildcard imports to be dynamically executed.

### Types
```rs
enum ImportKind {
    // Imports all the exported symbols in the form of an object
    Wildcard,
    // Imports only a given list of exports
    Specific(Vec<String>)
}

struct Import {
    // The module normalized path
    source: String,
    // Which type of import this is
    kind: ImportKind
}
```