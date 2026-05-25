# Texsch — Text-based Schematic Compiler

2D ASCII circuit schematics → SVG preview + KiCad S-expression files, in the browser.

```
U1: OPA330xxD
R1: R
R2: R
R3: R
J1: Conn_01x03_Socket
J2: Conn_Coaxial
J3: Conn_Coaxial
J4: Conn_Coaxial
#GND1: GND
#GND2: GND
#GND3: GND
#GND4: GND
#VCC1: VCC
#VSS1: VSS
=============================================
[In1]--J3:1<                                                       #VCC1:1v
        J3:2v                                                      |
        |                                                          U1:7(V+)^
        #GND3:1^                  +------------------------------U1:3(+)<
                                  |                                       U1:6>--------*--------+--[OUT]
                                  #GND1:1^             +---------U1:2(-)<              |        |
[In2]--J4:1<                                           |           U1:4(V-)v           |        |
        J4:2v                                          |           |                   |        +--J2:1<
        |                                              |           #VSS1:1^            |            J2:2v
        #GND4:1^            [In1]--R1:1< R1:2>---------*                               |            |
                                                       |                               |            #GND2:1^
[VCC]--J1:1<                [In2]--R2:1< R2:2>---------*                               |
[GND]--J1:2<                                           |                               |
[VSS]--J1:3<                                           +------------------R3:1< R3:2>--*
```

A grid of text produces a fully-routed schematic with standard KiCad library symbols — opamps, resistors, connectors, and power symbols — ready to open in KiCad or embed anywhere as SVG.

## Features

- **2D ASCII schematics**: describe circuits with a spatial grid — components, wires, junctions, and labels placed freely
- **Live preview**: Monaco-powered editor with instant WASM compilation on every keystroke
- **Dual output**: SVG render for visualization + KiCad `.kicad_sch` S-expression for EDA import
- **Standard KiCad library symbols**: `Device:R/L/C`, `Amplifier_Operational:OPA330xxD`, `Connector:Conn_Coaxial`, `Connector:Conn_01x03_Socket`, `power:GND/VCC/VSS`
- **Multi-pin symbols**: opamps and connectors with arbitrary pin counts and orientations
- **Auto-orientation**: components automatically rotate to match their grid placement
- **Power symbols**: `#`-prefixed refdes create global power symbols (GND, VCC, VSS)
- **Zero server**: everything runs client-side via WebAssembly (Rust → WASM)

## Quick Start

### Web App

```bash
cd web
cnpm install
cnpm run dev
```

Open the URL printed by Vite (default `http://localhost:5173`). The default schematic shows a summing amplifier (adder) circuit. Edit in the left pane, see SVG and KiCad output on the right.

### Native CLI (Rust)

```bash
cd core
cargo test    # run 94 tests
cargo build   # compile native binary
```

### Rebuild WASM

```bash
./build-wasm.sh
```

## Syntax — Port-based Grid Architecture

A schematic has two sections: a **header** declaring which symbol each refdes uses, and a **body** containing the 2D ASCII grid.

### Header

```
REFDES: SYMBOL_NAME
```

Each line maps a reference designator to its symbol. The header ends with a line of `=` characters (at least 3).

```
U1: OPA330xxD
R1: R
#GND1: GND
==========
```

Power symbols use a `#` prefix on the refdes (e.g. `#GND1`, `#VCC1`).

### Body — Nodes

Place **nodes** at specific character positions and connect them with wire characters.

| Token | Node Type | Description |
|-------|-----------|-------------|
| `[NAME]` | Label | Net label, e.g. `[VCC]`, `[GND]`, `[OUT]`, `[In1]` |
| `R1:1<`, `R1:2>` | Port | Component pin — pin number followed by direction (`<` `>` `^` `v`) |
| `U1:3(+)<` | Port | Pin with optional name in parentheses |
| `#GND1:1^` | Power Port | Power-symbol pin with `#`-prefixed refdes |
| `*` | Junction | Electrical connection dot (wire intersection) |
| `+` | Corner | Wire corner or crossing without electrical connection |

Port directions are **required**: `<` (left), `>` (right), `^` (up), `v` (down).

### Wires

Use `-` (dash) for horizontal wires between nodes on the same row. Use `|` (pipe) for vertical wires between nodes on the same column. Wires are detected by scanning for these characters between adjacent nodes.

### Topology Examples

**Horizontal resistor** (angle 90):
```
R1:1<  R1:2>
```

**Vertical capacitor** (angle 0):
```
C1:1^
C1:2v
```

**T-Junction**:
```
[VCC] ------- * ------- [OUT]
              |
              R2:1<
```

**Opamp in standard layout** (angle 0):
```
           U1:7(V+)^
U1:3(+)<
                  U1:6>
U1:2(-)<
           U1:4(V-)v
```

**Power symbol wiring**:
```
#VCC1:1v
|
U1:7(V+)^
```

## Architecture

```
texsch/
├── core/               Rust compiler crate
│   ├── src/
│   │   ├── lib.rs      WASM exports + compile() pipeline
│   │   ├── parser.rs   Scanner, compression, matching, spans, layout, wire extraction
│   │   ├── svg.rs      SVG renderer (symbols, wires, junction dots, span debug)
│   │   ├── kicad.rs    KiCad S-expression generator (symbols, wires, junctions)
│   │   └── kicad_sym.rs  KiCad symbol file parser + built-in library loader
│   ├── library/        Built-in KiCad symbol files
│   │   ├── Amplifier_Operational/
│   │   ├── Connector/
│   │   ├── Device/
│   │   └── power/
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

**Pipeline**: ASCII text → `parse_header` + `scan_nodes` → `compress_coordinates` → `match_components` → `solve_orientations` → `compute_spans` → `compute_layout` → `compute_pin_ki_positions` + `extract_wires` → SVG + KiCad output

### Steps

1. **Header** — parse `REFDES: SYMBOL` mappings from the header section
2. **Scan** — walk the 2D character grid, identify Labels, Junctions, Corners, and Ports with directions
3. **Compress** — map sparse absolute coordinates to dense grid indices
4. **Match** — group ports by refdes, validate against symbol library templates
5. **Orient** — solve component rotation angles from grid pin positions; auto-detect orientation
6. **Span** — compute four-direction bounding boxes for each node based on type and orientation
7. **Layout** — dynamic grid spacing (`col_x`, `row_y`) from span extents with configurable spacing
8. **Wires** — grid-neighbor routing: connect adjacent nodes on same row if `-` present, same column if `|` present
9. **Render** — SVG (symbols, wires, junction dots) and KiCad S-expression (symbols, wires, junctions, labels)

## Built-in Symbol Library

| Symbol | Library | Pins | Description |
|--------|---------|------|-------------|
| `R` | Device | 2 | Resistor (IEC) |
| `C` | Device | 2 | Capacitor (unpolarized) |
| `L` | Device | 2 | Inductor (IEC) |
| `OPA330xxD` | Amplifier_Operational | 8 | Precision CMOS opamp (SOIC-8) |
| `Conn_Coaxial` | Connector | 2 | Coaxial connector (BNC/SMA) |
| `Conn_01x03_Socket` | Connector | 3 | 3-pin socket header |
| `GND` | power | 1 | Earth ground symbol |
| `VCC` | power | 1 | Positive power rail |
| `VSS` | power | 1 | Negative power rail |

Adding a new symbol is zero-code: drop a `.kicad_sym` file into the appropriate `library/` subdirectory and rebuild.

## KiCad Compatibility

Generated `.kicad_sch` files use the KiCad S-expression format (version 20260306). Symbols reference standard KiCad libraries, so schematics open directly in KiCad without custom library setup. Component rotation, property positions (Reference, Value, Footprint, Datasheet), and pin connections are all emitted correctly.

## License

MIT
