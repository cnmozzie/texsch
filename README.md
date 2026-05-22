# Texsch — Text-based Schematic Compiler

2D ASCII circuit schematics → SVG preview + KiCad S-expression files, in the browser.

```
[VCC] ------- * ------- [OUT]
              |
              R2:1
              R2:2
```

A grid of text produces a fully-routed schematic with standard KiCad library symbols, ready to open in KiCad or embed anywhere as SVG.

## Features

- **2D ASCII schematics**: describe circuits with a spatial grid — components, wires, junctions, and labels placed freely
- **Live preview**: Monaco-powered editor with instant WASM compilation on every keystroke
- **Dual output**: SVG render for visualization + KiCad `.kicad_sch` S-expression for EDA import
- **Standard library symbols**: uses `Device:R`, `Device:C`, `Device:L` from KiCad's built-in library
- **Zero server**: everything runs client-side via WebAssembly (Rust → WASM)
- **MVP component set**: Resistor (R), Capacitor (C), Inductor (L)

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
cargo test    # run 48 tests
cargo build   # compile native binary
```

### Rebuild WASM

```bash
./build-wasm.sh
```

## Syntax — Port-based Grid Architecture

A schematic is a 2D text grid. Place **nodes** at specific character positions, and connect them with wire characters (`-` for horizontal, `|` for vertical).

### Nodes

| Token | Node Type | Description |
|-------|-----------|-------------|
| `[NAME]` | Label | Net label, e.g. `[VCC]`, `[GND]`, `[OUT]` |
| `R1:1`, `R1:2` | Port | Component pin — pin 1 and pin 2 of refdes |
| `C1:1`, `C1:2` | Port | Capacitor pins |
| `L1:1`, `L1:2` | Port | Inductor pins |
| `*` | Junction | Electrical connection dot (4-way intersection) |
| `+` | Corner | Wire crossing without electrical connection |

### Wires

Use `-` (dash) for horizontal wires between nodes on the same row. Use `|` (pipe) for vertical wires between nodes on the same column.

### Component Placement

Components are formed by two ports of the same refdes on **adjacent** grid positions:
- Same row, adjacent columns → **Horizontal** component
- Same column, adjacent rows → **Vertical** component

### Topology Examples

**T-Junction** (junction dot at `*`):
```
[VCC] ------- * ------- [OUT]
              |
              R2:1
              R2:2
```

**Corner** (no dot at `+`):
```
[VCC] ---+
         |
         R1:1
         R1:2
```

**Crossing without Connection** (no dot at `+`):
```
         [Y1]
          |
[X1] ----+---- [X2]
          |
         [Y2]
```

**Cross-Junction** (junction dot at `*`):
```
          [UP]
           |
[LEFT] --- * --- [RIGHT]
           |
         [DOWN]
```

## Architecture

```
texsch/
├── core/               Rust compiler crate
│   ├── src/
│   │   ├── lib.rs      WASM exports + compile() pipeline
│   │   ├── parser.rs   Scanner, compression, pairing, spans, layout, wire extraction
│   │   ├── svg.rs      SVG renderer (symbols, wires, junction dots, span debug)
│   │   └── kicad.rs    KiCad S-expression generator (symbols, wires, junctions)
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

**Pipeline**: ASCII text → `scan_nodes` → `compress_coordinates` → `pair_components` → `compute_spans` → `compute_layout` → `extract_wires` → SVG + KiCad output

### Steps

1. **Scan** — walk the 2D character grid, identify Labels `[...]`, Junctions `*`, Corners `+`, and Ports `R1:1`
2. **Compress** — map sparse absolute coordinates to dense grid indices
3. **Pair** — match port pairs by refdes, validate adjacency, determine orientation
4. **Span** — compute four-direction bounding boxes for each node based on type and orientation
5. **Layout** — dynamic grid spacing (`col_x`, `row_y`) from span extents with configurable `MIN_GAP`
6. **Wires** — grid-neighbor routing: connect adjacent nodes on same row if `-` present, same column if `|` present
7. **Render** — SVG (symbols, wires, junction dots, span debug boxes) and KiCad S-expression (symbols, wires, junctions)

## KiCad Compatibility

Generated `.kicad_sch` files use the KiCad S-expression format. Symbols reference KiCad's standard `Device` library, so the schematic opens directly in KiCad without custom library setup. Wire endpoints use correct pin offsets (3.81 mm) matching the Device symbol definitions.

## License

MIT
