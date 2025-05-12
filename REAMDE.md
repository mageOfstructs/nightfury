# Nightfury

- automatic language-independent syntax-insertion engine relying on syntax

## Project Goal

The primary goal is to minimise the time programmers spent on typing syntax. It's aim is not to (fully) replace LSP, but to work alongside it.

## Features

- [ ] BNF-based tree generator
  - needs more testing
- [ ] IDE-independent client-server architecture
- [ ] lots of customization-potential

## Running

```sh
cargo run # debug build, very verbose logging
```

```sh
cargo run --release # release build, almost no logging
```
