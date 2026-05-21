# Texsch — Text-based Schematic Compiler

ASCII circuit schematics → SVG preview + KiCad S-expression files, in the browser.

```
GND --- -R1(24.9)- --- -C1(10u)- --- VCC
```

A single line of text produces a fully-routed schematic with standard KiCad library symbols, ready to open in KiCad or embed anywhere as SVG.

## Features

- **Text-first**: describe circuits with a compact ASCII syntax — one line = one signal chain
- **Live preview**: Monaco-powered editor with instant WASM compilation on every keystroke
- **Dual output**: SVG render for visualization + KiCad `.kicad_sch` S-expression for EDA import
- **Standard library symbols**: uses `Device:R`, `Device:C`, `Device:L` from KiCad's built-in library
- **Zero server**: everything runs client-side via WebAssembly (Rust → WASM)
- **MVP component set**: Resistor (R), Capacitor (C), Inductor (L) with refdes and value support

## Quick Start

### Web App

```bash
cd web
cnpm install
cnpm run dev
```

Open the URL printed by Vite (default `http://localhost:5173`). Type a schematic in the left pane, see SVG and KiCad output on the right.

### Native CLI (Rust)

```bash
cd core
cargo run --example smoke
```

Produces `example_output.svg` and `example_output.kicad_sch` in the project root.

### Rebuild WASM

```bash
./build-wasm.sh
```

Requires `rustc` 1.95+, `wasm32-unknown-unknown` target, and `wasm-bindgen` CLI on `$PATH`.

## Syntax

A schematic line is a sequence of **labels**, **wires**, and **components** separated by whitespace.

### Labels

Any bare word that is not a wire or component is a net label:

```
GND    VCC    VIN    OUT    NET1
```

### Wires

One or more consecutive dashes connect adjacent elements:

```
---    ------    -
```

Longer wires create more visual spacing; electrically they are transparent.

### Components

```
-<REFDES>(<VALUE>)-
```

| Element  | Meaning          | Example   |
| -------- | ---------------- | --------- |
| `R1`     | Resistor refdes  | `-R1(10k)-` |
| `C3`     | Capacitor refdes | `-C3(47u)-` |
| `L2`     | Inductor refdes  | `-L2(4.7mH)-` |

The refdes must start with `R`, `C`, or `L` followed by at least one digit. The value can be any text (conventionally `10k`, `47u`, `4.7mH`, etc.).

### Examples

```
GND --- -R1(24.9)- --- -C1(10u)- --- VCC
```

```
-L1(10mH)- --- -C2(47u)- --- GND
```

```
VIN --- -R1(1k)- --- -C1(100n)- --- -L1(10uH)- --- OUT
```

## Architecture

```
texsch/
├── core/               Rust compiler crate
│   ├── src/
│   │   ├── lib.rs      WASM exports + core types
│   │   ├── parser.rs   Lexer & linear-layout parser
│   │   ├── svg.rs      SVG renderer
│   │   └── kicad.rs    KiCad S-expression generator
│   ├── examples/
│   │   └── smoke.rs    CLI demo
│   └── Cargo.toml
├── web/                Vue 3 + Vite + TypeScript frontend
│   ├── src/
│   │   ├── App.vue     Split-pane editor (Monaco + WASM)
│   │   ├── wasm/       Generated WASM bindings (by build-wasm.sh)
│   │   └── main.ts     App entry point
│   └── package.json
├── build-wasm.sh       Build Rust → WASM → web/src/wasm/
└── README.md
```

**Pipeline**: ASCII text → `parser::tokenize` → `parser::build_circuit` → `Circuit` IR → `svg::generate` + `kicad::generate`

## Building from Source

### Prerequisites

- Rust 1.95+ with `wasm32-unknown-unknown` target
- `wasm-bindgen` CLI (`cargo install wasm-bindgen-cli`)
- Node.js 18+ with `cnpm`

### All targets

```bash
# Native Rust
cd core && cargo build

# Run tests
cd core && cargo test

# WASM
./build-wasm.sh

# Web frontend
cd web && cnpm install && cnpm run build
```

## KiCad Compatibility

The generated `.kicad_sch` files use the KiCad S-expression format. Symbols reference KiCad's standard `Device` library (`Device:R`, `Device:C`, `Device:L`), so the schematic opens directly in KiCad without any custom library setup.

## License

MIT
