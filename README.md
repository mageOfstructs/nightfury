# Nightfury

- automatic language-independent autocompletion engine relying on EBNF syntax

## Project Goal

The primary goal is to minimise the time programmers spent on typing syntax. Its aim is not to (fully) replace LSP, but to work alongside it.

## Concept

For instance, take this variable declaration in Java: `double PI;`.
You would start typing this by first pressing the letter 'd'. Nightfury sees that and compares it against an internal data structure. If only the token `double` is possible at the location in the code, it'll automatically autocomplete it for you. But what now? The Java syntax demands a variable name here, a string which the programmer is free to choose themselves. Nightfury can't (and won't) interfere with that, instead quietly listening for a token that terminates the identifier, a semicolon in this case.

To at least try to be language-independent, the project uses the `ebnf` parser crate as well as a custom translator to convert conventional EBNF-diagrams into its internal data structure.

## Features

- [x] EBNF-based tree generator
- [x] IDE-independent client-server architecture
- [ ] lots of customization-potential

## Running

```sh
cargo run # debug build, very verbose logging
```

```sh
cargo run --release # release build, almost no logging (i.e. what you should use)
```

## Reading the FSM

Run the program in debug mode. Most of its output is debugging information, which will be documented *someday*. For now, you just need the section after `FSM:`. This is the FSM nightfury generated from the provided ebnf. It currently supports three types of nodes:

- `Keyword`: some keyword, has two important fields: `expanded` (the actual keyword) and `short` (the character sequence you need to type for it to be autocompleted)
  - Note: if nightfury can definitely determine what keyword should be inserted before you finish typing the entire short-sequence, it will insert it without needing you to finish typing the `short` sequence
- `UserDefinedCombo`: section for a user-defined token, e.g. identifiers. Consists of a regex (used for deciding which branch to take) and an array of characters called "final_tokens" (used to determine when the userdefined token is completed)
- `Null`: placeholder node, used to either combine paths or split them apart

The indentation shows you the general flow of the graph. If you see a "Cycle to ID", then that means there is a node link that cannot cleanly be displayed in the tree view (e.g. cycles)

## Architecture

- `nightfury`: the main lib crate; provides the main API for completions
- `nigthfury-server`: server frontend that can take commands in JSON-Format over a UNIX socket and manipulate the internal FSMs
- `nightfury-cli`: cli containing helper methods for generating nightfury fsms as well as server debugging
- `nightfury-vscode`: source code for the visual studio integration extension
