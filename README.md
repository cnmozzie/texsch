# Texsch — Text-based Schematic Compiler

2D ASCII circuit schematics → SVG preview + KiCad S-expression files, in the browser.

```
U1: OPA330xxD
R1: R
R2: R
R3: R
R4: R
J1: Conn_01x03_Socket
J2: Conn_Coaxial
J3: Conn_Coaxial
J4: Conn_Coaxial
#VCC1: VCC
#VSS1: VSS
#GND1: GND
#GND2: GND
#GND3: GND
#GND4: GND
=============================================

 [in1] --<J3                                #VCC1
          v                                 v
          |                                 |
          ^                                 ^
          #GND3             +--------------<
                            |                U1>----*-----+--[OUT]
 [in2]---<J4                ^         +--- <        |     |
          v                 #GND2     |     v       |     |
          |            [In1]--<R1> -- *     |       |     |
          ^                           |     ^       |     +--<J2
          #GND4        [In2]--<R2> -- *     #VSS1   |         v
                                      |             |         |
                                      +------<R3>---+         ^
 [VCC]----<                                                   #GND1
 [GND]----<J1
 [VSS]----<
=============================================
```

A grid of text produces a fully-routed schematic with standard KiCad library symbols — opamps, resistors, connectors, and power symbols — ready to open in KiCad or embed anywhere as SVG.

## Features

- **2D ASCII schematics**: describe circuits with a spatial grid — components, wires, junctions, and labels placed freely
- **Live preview**: Monaco-powered editor with instant WASM compilation on every keystroke
- **Dual output**: SVG render for visualization + KiCad `.kicad_sch` S-expression for EDA import
- **Standard KiCad library symbols**: `Device:R/L/C`, `Amplifier_Operational:OPA330xxD`, `Connector:Conn_Coaxial`, `Connector:Conn_01x03_Socket`, `power:GND/VCC/VSS`
- **Multi-pin symbols**: opamps and connectors with arbitrary pin counts and orientations
- **Auto-orientation**: components automatically solve rotation angle from grid placement (DAG solver with rigid rel_phys constraints)
- **Dual Grid**: main circuit (Grid1) + component preview sandbox (Grid2), electrically isolated
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
cargo test    # run 97 tests
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

Each line maps a reference designator to its symbol. The header ends with the first line of `=` characters (at least 3). A second `====` separator can split the body into **Grid1** (main circuit) and **Grid2** (component preview / sandbox). Only Grid1 contributes to KiCad output.

```
U1: OPA330xxD
R1: R
#GND1: GND
==========
```

Power symbols use a `#` prefix on the refdes (e.g. `#GND1`, `#VCC1`).

### Body — Nodes

Place **nodes** at specific character positions and connect them with wire characters. Each component occupies a **text-grid footprint**: the refdes text sits at the anchor position, and arrow characters (`<` `>` `^` `v`) placed at template-defined offsets mark its pins.

| Token | Node Type | Description |
|-------|-----------|-------------|
| `[NAME]` | Label | Net label, e.g. `[VCC]`, `[GND]`, `[OUT]`, `[In1]` |
| `>`, `<`, `^`, `v` | Port | Arrow character defining a component pin with direction (template offset from refdes text) |
| `*` | Junction | Electrical connection dot (wire intersection) |
| `+` | Corner | Wire corner or crossing without electrical connection |
| `.` | Placeholder | Empty cell preserving grid spacing (no wire) |

Arrow directions: `<` (left), `>` (right), `^` (up), `v` (down). The refdes text (e.g. `U1`, `R2`) sits at the component's anchor position, and arrow characters are placed around it according to the symbol template. The WASM `get_rotated_footprint` function returns the exact cell offsets for each rotation angle.

### Wires

Use `-` (dash) for horizontal wires between nodes on the same row. Use `|` (pipe) for vertical wires between nodes on the same column. Wires are detected by scanning for these characters between adjacent nodes.

### Topology Examples

**Horizontal resistor** (angle 0):
```
<R1>
```

**Vertical resistor** (angle 90):
```
 ^
R1
 v
```

**T-Junction**:
```
[VCC] ------- * ------- [OUT]
              |
             <R2
```

**Opamp in standard layout** (angle 0):
```
 ^
<
  U1>
<
 v
```

**Power symbol wiring**:
```
#VCC1
v
|
^
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

**Pipeline**: ASCII text → `parse_header` + `scan_nodes` → `compress_coordinates` → `match_components` → `solve_orientations` → `apply_rotation_to_rel_phys` → `compute_spans` → `compute_layout` → `compute_pin_ki_positions` + `extract_wires` → SVG + KiCad output

### Steps

1. **Header** — parse `REFDES: SYMBOL` mappings from the header section; split into three sections (header, Grid1 body, Grid2 body)
2. **Scan** — walk the 2D character grid, identify Labels, Junctions, Corners, Ports, and Placeholders
3. **Compress** — map sparse absolute coordinates to dense grid indices
4. **Match** — group port arrows by refdes, validate against symbol library templates; resolve pin grid positions
5. **Orient** — solve component CW rotation angle from feature-pin grid positions
6. **Rotate** — apply rotation to template rel_phys_x/y values for DAG constraints
7. **Span** — compute four-direction bounding boxes for each node (rigid component constraints override span)
8. **Layout** — DAG longest-path solver with independent col_x/row_y per grid; backward pass for pin-to-anchor spacing
9. **Pin KiCad** — map pin KiCad positions from grid coords (ensures straight wires aligned with grid)
10. **Wires** — grid-neighbor routing with endpoint snapping to port/label edges
11. **Render** — SVG (dual-grid stacked: Grid1, dashed separator, Grid2) and KiCad S-expression (Grid1 only)

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
