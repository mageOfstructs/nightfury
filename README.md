# Nightfury

- automatic language-independent syntax-insertion engine relying on syntax

## Project Goal

The primary goal is to minimise the time programmers spent on typing syntax. It's aim is not to (fully) replace LSP, but to work alongside it.

## Concept

For instance, take this variable declaration in Java: `double PI;`.
You would start typing this by first pressing the letter 'd'. Nightfury sees that and compares it against an internal data structure. If only the token `double` is possible at the location in the code, it'll automatically autocomplete it for you. But what now? The Java syntax demands a variable name here, a string which the programmer is free to choose themselves. Nightfury can't (and won't) interfere with that, instead quietly listening for a token that terminates the identifier, a semicolon in this case.

To at least try to be language-independent, the project uses the `ebnf` parser crate as well as a custom translator to convert conventional EBNF-diagrams into its internal data structure.

## Features

- [ ] EBNF-based tree generator
  - constructs seem to work fine on their own
  - needs more testing to catch edge cases when combining
- [ ] IDE-independent client-server architecture
- [ ] lots of customization-potential

## Running

```sh
cargo run # debug build, very verbose logging
```

```sh
cargo run --release # release build, almost no logging (i.e. what you should use)
```

## Architecture

- `lib` crate: actual logic, largely client-independent
- `bin` crate: demo app, will be made into an example in the future
