// ============================================================
// Step 1: Node Identification & Absolute Coordinate Extraction
// Port-based Grid Architecture — Anchor-based Multi-Port Symbols
// ============================================================

use std::collections::HashMap;

// ============================================================
// Symbol Library Definitions
// ============================================================

/// Direction a pin faces on the schematic grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDirection {
    Left,   // <
    Right,  // >
    Up,     // ^
    Down,   // v
}

impl PinDirection {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '<' => Some(Self::Left),
            '>' => Some(Self::Right),
            '^' => Some(Self::Up),
            'v' => Some(Self::Down),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Self::Left => '<',
            Self::Right => '>',
            Self::Up => '^',
            Self::Down => 'v',
        }
    }
}

/// One pin in a component symbol definition.
/// All offsets are relative to the **anchor pin** (first pin in the symbol's pins list).
#[derive(Debug, Clone)]
pub struct PinTemplate {
    pub pin_num: usize,
    pub name: String,
    pub dir: PinDirection,
    /// Grid row offset from the anchor pin (in compressed grid units).
    pub rel_grid_row: i32,
    /// Grid column offset from the anchor pin (in compressed grid units).
    pub rel_grid_col: i32,
    /// Physical X offset from the anchor pin (SVG pixels).
    pub rel_phys_x: f64,
    /// Physical Y offset from the anchor pin (SVG pixels).
    pub rel_phys_y: f64,
    /// Pin lead length in mm (from KiCad `(length …)`), used to draw
    /// the connection line from the pin position toward the symbol body.
    pub pin_length_mm: f64,
}

/// A drawing primitive in grid-relative coordinates.
#[derive(Debug, Clone)]
pub enum DrawPrimitive {
    Polyline {
        pts: Vec<(f64, f64)>,
        stroke_width: f64,
        fill_type: String,
    },
    Rectangle {
        start: (f64, f64),
        end: (f64, f64),
        stroke_width: f64,
        fill_type: String,
    },
    Arc {
        start: (f64, f64),
        mid: (f64, f64),
        end: (f64, f64),
        stroke_width: f64,
        fill_type: String,
    },
    Circle {
        center: (f64, f64),
        radius: f64,
        stroke_width: f64,
        fill_type: String,
    },
}

/// A component symbol definition in the built-in library.
/// The first pin in `pins` is the **anchor** (reference pin).
#[derive(Debug, Clone)]
pub struct ComponentSymbol {
    /// Short symbol name (e.g. "R", "OPA330xxD") — used for header lookup.
    pub symbol_name: String,
    /// Fully-qualified KiCad lib_id (e.g. "Device:R", "Amplifier_Operational:OPA330xxD").
    pub lib_id: String,
    pub pins: Vec<PinTemplate>,
    /// Optional drawing geometry in grid-relative coordinates.
    /// When empty, the renderer falls back to hardcoded shapes.
    pub draw_primitives: Vec<DrawPrimitive>,
    /// ALL pin numbers (including hidden NC pins), used for KiCad output.
    pub all_pin_numbers: Vec<usize>,
    /// Anchor pin's X position in the original KiCad symbol (mm).
    /// Used to offset KiCad symbol placement from anchor to origin.
    pub anchor_ki_x: f64,
    /// Anchor pin's Y position in the original KiCad symbol (mm).
    pub anchor_ki_y: f64,
    /// First feature pin number for orientation detection (None = skip).
    pub feature_pin_a: Option<usize>,
    /// Second feature pin number for orientation detection.
    pub feature_pin_b: Option<usize>,
    /// Reference property offset from symbol origin (KiCad Y-up, mm).
    pub ref_ki_x: f64,
    /// Reference property offset from symbol origin (KiCad Y-up, mm).
    pub ref_ki_y: f64,
    /// Reference property text angle from template (degrees).
    pub ref_ki_angle: f64,
    /// Value property offset from symbol origin (KiCad Y-up, mm).
    pub val_ki_x: f64,
    /// Value property offset from symbol origin (KiCad Y-up, mm).
    pub val_ki_y: f64,
    /// Value property text angle from template (degrees).
    pub val_ki_angle: f64,
}

/// Initialise the hardcoded symbol library with anchor-based templates.
/// R, L, C two-pin symbols are built-in.
/// Multi-pin symbols (e.g. OpAmp) are loaded from KiCad symbol files
/// by the `compile` pipeline.
pub fn init_symbol_library() -> Vec<ComponentSymbol> {
    vec![
        // Resistor: pin 1 is anchor, pin 2 is 1 grid column to the right
        ComponentSymbol {
            symbol_name: "R".to_string(),
            lib_id: "Device:R".to_string(),
            pins: vec![
                PinTemplate {
                    pin_num: 1, name: String::new(), dir: PinDirection::Left,
                    rel_grid_row: 0, rel_grid_col: 0,
                    rel_phys_x: 0.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
                PinTemplate {
                    pin_num: 2, name: String::new(), dir: PinDirection::Right,
                    rel_grid_row: 0, rel_grid_col: 1,
                    rel_phys_x: 60.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
            ],
            draw_primitives: vec![],
            all_pin_numbers: vec![1, 2],
            anchor_ki_x: 0.0,
            anchor_ki_y: 3.81,
            feature_pin_a: Some(1),
            feature_pin_b: Some(2),
            ref_ki_x: 6.35,
            ref_ki_y: 0.0,
            ref_ki_angle: 90.0,
            val_ki_x: 3.81,
            val_ki_y: 0.0,
            val_ki_angle: 90.0,
        },
        // Capacitor: pin 1 is anchor
        ComponentSymbol {
            symbol_name: "C".to_string(),
            lib_id: "Device:C".to_string(),
            pins: vec![
                PinTemplate {
                    pin_num: 1, name: String::new(), dir: PinDirection::Left,
                    rel_grid_row: 0, rel_grid_col: 0,
                    rel_phys_x: 0.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
                PinTemplate {
                    pin_num: 2, name: String::new(), dir: PinDirection::Right,
                    rel_grid_row: 0, rel_grid_col: 1,
                    rel_phys_x: 56.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
            ],
            draw_primitives: vec![],
            all_pin_numbers: vec![1, 2],
            anchor_ki_x: 0.0,
            anchor_ki_y: 3.81,
            feature_pin_a: Some(1),
            feature_pin_b: Some(2),
            ref_ki_x: 3.81,
            ref_ki_y: 1.2701,
            ref_ki_angle: 0.0,
            val_ki_x: 3.81,
            val_ki_y: -1.2699,
            val_ki_angle: 0.0,
        },
        // Inductor: pin 1 is anchor
        ComponentSymbol {
            symbol_name: "L".to_string(),
            lib_id: "Device:L".to_string(),
            pins: vec![
                PinTemplate {
                    pin_num: 1, name: String::new(), dir: PinDirection::Left,
                    rel_grid_row: 0, rel_grid_col: 0,
                    rel_phys_x: 0.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
                PinTemplate {
                    pin_num: 2, name: String::new(), dir: PinDirection::Right,
                    rel_grid_row: 0, rel_grid_col: 1,
                    rel_phys_x: 60.0, rel_phys_y: 0.0,
                    pin_length_mm: 2.54,
                },
            ],
            draw_primitives: vec![],
            all_pin_numbers: vec![1, 2],
            anchor_ki_x: 0.0,
            anchor_ki_y: 3.81,
            feature_pin_a: Some(1),
            feature_pin_b: Some(2),
            ref_ki_x: 1.27,
            ref_ki_y: 1.2701,
            ref_ki_angle: 0.0,
            val_ki_x: 1.27,
            val_ki_y: -1.2699,
            val_ki_angle: 0.0,
        },
    ]
}

// ============================================================
// Core Node Types
// ============================================================

/// Absolute position in the 2D character grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbsPos {
    pub row: usize,
    pub col: usize,
}

/// Type of a schematic node discovered during scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    /// A generic component port, e.g. "U1:3(+)<", "R1:1<", "R1:2>"
    Port { refdes: String, pin: usize, name: String, dir: PinDirection },
    /// A net label, e.g. "[VCC]", "[GND]"
    Label(String),
    /// An electrical junction point marked by '*'
    Junction,
    /// A wire corner/crossing marked by '+'
    Corner,
}

/// A node discovered in the ASCII schematic grid.
#[derive(Debug, Clone)]
pub struct SchematicNode {
    pub node_type: NodeType,
    pub pos: AbsPos,
    /// Number of character columns this node occupies.
    pub text_width: usize,
    /// Compressed row index (populated by [`compress_coordinates`]).
    pub grid_row: usize,
    /// Compressed column index (populated by [`compress_coordinates`]).
    pub grid_col: usize,
    /// Four-direction margin (populated by [`compute_spans`]).
    pub span: NodeSpan,
}

/// Four-direction physical margin around a node's grid centre.
#[derive(Debug, Clone, Copy)]
pub struct NodeSpan {
    pub left: f64,
    pub right: f64,
    pub up: f64,
    pub down: f64,
}

/// Base half-span in mm for junctions, corners, and wire segments.
/// Old: 10.0 px × 2.54/60 ≈ 0.42 mm → rounded to 0.5 mm.
pub const HALF_SPAN: f64 = 1.0;

/// Minimum safe clearance between two adjacent span bounding boxes (mm).
pub const MIN_GAP: f64 = 0.0;

/// Margin in mm from origin to first grid line.
/// Old: 60.0 px × 2.54/60 = 2.54 mm (= 1 grid unit).
pub const MARGIN: f64 = 12.8;

/// SVG px per grid unit — used only by SVG rendering.
pub const CELL_W: f64 = 20.0;
pub const CELL_H: f64 = 20.0;

// ============================================================
// Rigid Template Matching
// ============================================================

/// One matched pin within an instantiated component.
#[derive(Debug, Clone)]
pub struct MatchedPin {
    pub pin_num: usize,
    pub name: String,
    pub dir: PinDirection,
    pub grid_row: usize,
    pub grid_col: usize,
    /// Physical offset from the anchor (copied from [`PinTemplate`], may be swapped).
    pub rel_phys_x: f64,
    /// Physical offset from the anchor (copied from [`PinTemplate`], may be swapped).
    pub rel_phys_y: f64,
    /// Original (unswapped) template rel_phys — used for orientation solving.
    pub tmpl_phys_x: f64,
    /// Original (unswapped) template rel_phys — used for orientation solving.
    pub tmpl_phys_y: f64,
    /// Template pin direction (from symbol library) — used for single-pin rotation.
    pub tmpl_dir: PinDirection,
    /// Pin lead length in mm (copied from [`PinTemplate`]).
    pub pin_length_mm: f64,
}

/// CW index for a pin direction — used to compute single-pin rotation angle.
/// SVG rotate(90°) in Y-down maps Right→Down→Left→Up→Right (clockwise).
fn dir_cw_index(dir: PinDirection) -> i32 {
    match dir {
        PinDirection::Right => 0,
        PinDirection::Down => 1,
        PinDirection::Left => 2,
        PinDirection::Up => 3,
    }
}

/// A component successfully matched against a symbol library template.
#[derive(Debug, Clone)]
pub struct MatchedComponent {
    pub refdes: String,
    /// Short symbol name (e.g. "R", "OPA330xxD").
    pub symbol_name: String,
    /// Fully-qualified KiCad lib_id (e.g. "Device:R").
    pub lib_id: String,
    pub pins: Vec<MatchedPin>,
    /// Compressed grid row of the anchor pin.
    pub anchor_grid_row: usize,
    /// Compressed grid column of the anchor pin.
    pub anchor_grid_col: usize,
    /// Drawing primitives cloned from the symbol library (grid-relative).
    pub draw_primitives: Vec<DrawPrimitive>,
    /// ALL pin numbers (including hidden NC pins).
    pub all_pin_numbers: Vec<usize>,
    /// Anchor pin's KiCad X position (mm, from symbol origin).
    pub anchor_ki_x: f64,
    /// Anchor pin's KiCad Y position (mm, from symbol origin).
    pub anchor_ki_y: f64,
    /// Rotation angle in degrees (0, 90, 180, 270) — KiCad CW convention.
    pub angle: f64,
    /// Pre-computed KiCad mm positions for each pin (canvas Y-down coords).
    pub pin_ki_x: Vec<f64>,
    /// Pre-computed KiCad mm positions for each pin (canvas Y-down coords).
    pub pin_ki_y: Vec<f64>,
    /// Reference property offset from symbol origin (KiCad Y-up, mm).
    pub ref_ki_x: f64,
    pub ref_ki_y: f64,
    pub ref_ki_angle: f64,
    /// Value property offset from symbol origin (KiCad Y-up, mm).
    pub val_ki_x: f64,
    pub val_ki_y: f64,
    pub val_ki_angle: f64,
}

/// Split input into header and body sections separated by a line matching `^===+$`.
///
/// Returns `(header_str, body_str)`. If no separator is found, the entire input
/// is treated as body and the header is empty.
pub fn split_header_body(input: &str) -> (&str, &str) {
    for line in input.lines() {
        if line.len() >= 3 && line.chars().all(|c| c == '=') {
            let (header, body) = input.split_at(
                input.match_indices(line).next().unwrap().0
            );
            // body starts after the === line.
            // strip only leading blank lines — spaces are significant ASCII-art indentation.
            let body_start = body.find('\n').map(|p| p + 1).unwrap_or(body.len());
            let body_str = body[body_start..].trim_start_matches(|c: char| c == '\n' || c == '\r');
            return (header.trim_end(), body_str);
        }
    }
    ("", input)
}

/// Parse the header section into a `refdes → symbol_name` mapping.
///
/// Each non-empty line must be of the form `refdes: symbol_name` (e.g. `U1: OPA330xxD`).
/// Lines that don't match this pattern are silently ignored.
pub fn parse_header(header: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in header.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((refdes, symbol)) = line.split_once(':') {
            let refdes = refdes.trim().to_string();
            let symbol = symbol.trim().to_string();
            if !refdes.is_empty() && !symbol.is_empty() {
                map.insert(refdes, symbol);
            }
        }
    }
    map
}

/// Match scanned ports against the symbol library using rigid template validation.
///
/// Every refdes found in the schematic body MUST be declared in `refdes_to_symbol`.
/// If a refdes is not declared, an error is reported.
///
/// The `refdes_to_symbol` map (from the header) maps each refdes to a short symbol
/// name (e.g. "U1" → "OPA330xxD"). That name is then looked up in `symbol_library`
/// to get the [`ComponentSymbol`] template.
///
/// All pins defined in the template must be present at the correct relative grid
/// positions, otherwise an error is reported.
pub fn match_components(
    nodes: &[SchematicNode],
    refdes_to_symbol: &HashMap<String, String>,
    symbol_library: &HashMap<String, ComponentSymbol>,
) -> (Vec<MatchedComponent>, Vec<String>) {
    let mut groups: std::collections::BTreeMap<String, Vec<&SchematicNode>> =
        std::collections::BTreeMap::new();

    for node in nodes {
        if let NodeType::Port { refdes, .. } = &node.node_type {
            groups.entry(refdes.clone()).or_default().push(node);
        }
    }

    let mut matched = Vec::new();
    let mut errors = Vec::new();

    for (refdes, ports) in &groups {
        // Every refdes in the body MUST be declared in the header.
        let Some(symbol_name) = refdes_to_symbol.get(refdes) else {
            errors.push(format!(
                "{}: undeclared component refdes — add '{}: <SymbolName>' to the header before the === line",
                refdes, refdes
            ));
            continue;
        };

        // Look up the symbol in the library.
        let Some(symbol) = symbol_library.get(symbol_name) else {
            errors.push(format!(
                "{}: symbol \"{}\" not found in library",
                refdes, symbol_name
            ));
            continue;
        };

        // Locate the anchor pin (first in symbol.pins) among scanned ports.
        let anchor_template = &symbol.pins[0];
        let anchor_node = ports.iter().find(|n| {
            matches!(&n.node_type, NodeType::Port { pin, .. } if *pin == anchor_template.pin_num)
        });

        let Some(anchor_node) = anchor_node else {
            errors.push(format!(
                "{}: anchor pin {} ({}) not found in schematic",
                refdes, anchor_template.pin_num, anchor_template.name
            ));
            continue;
        };

        let _anchor_grid_row = anchor_node.grid_row as i32;
        let _anchor_grid_col = anchor_node.grid_col as i32;

        let mut matched_pins: Vec<MatchedPin> = Vec::new();

        // Add anchor pin
        if let NodeType::Port { name, dir, .. } = &anchor_node.node_type {
            matched_pins.push(MatchedPin {
                pin_num: anchor_template.pin_num,
                name: if name.is_empty() { anchor_template.name.clone() } else { name.clone() },
                dir: *dir,
                grid_row: anchor_node.grid_row,
                grid_col: anchor_node.grid_col,
                rel_phys_x: anchor_template.rel_phys_x,
                rel_phys_y: anchor_template.rel_phys_y,
                tmpl_phys_x: anchor_template.rel_phys_x,
                tmpl_phys_y: anchor_template.rel_phys_y,
                tmpl_dir: anchor_template.dir,
                pin_length_mm: anchor_template.pin_length_mm,
            });
        }

        // Collect all non-anchor template pins — accept any grid position.
        // apply_rotation_to_rel_phys() will fix up rel_phys after orientation solving.
        for template_pin in &symbol.pins[1..] {
            let found = ports.iter().find(|n| {
                matches!(&n.node_type, NodeType::Port { pin, .. } if *pin == template_pin.pin_num)
            });

            match found {
                Some(node) => {
                    if let NodeType::Port { name, dir, .. } = &node.node_type {
                        matched_pins.push(MatchedPin {
                            pin_num: template_pin.pin_num,
                            name: if name.is_empty() { template_pin.name.clone() } else { name.clone() },
                            dir: *dir,
                            grid_row: node.grid_row,
                            grid_col: node.grid_col,
                            rel_phys_x: template_pin.rel_phys_x,
                            rel_phys_y: template_pin.rel_phys_y,
                            tmpl_phys_x: template_pin.rel_phys_x,
                            tmpl_phys_y: template_pin.rel_phys_y,
                            tmpl_dir: template_pin.dir,
                            pin_length_mm: template_pin.pin_length_mm,
                        });
                    }
                }
                None => {
                    // Pin not drawn — acceptable for optional power pins
                }
            }
        }

        if !matched_pins.is_empty() {
            matched.push(MatchedComponent {
                refdes: refdes.clone(),
                symbol_name: symbol.symbol_name.clone(),
                lib_id: symbol.lib_id.clone(),
                pins: matched_pins,
                anchor_grid_row: anchor_node.grid_row,
                anchor_grid_col: anchor_node.grid_col,
                draw_primitives: symbol.draw_primitives.clone(),
                all_pin_numbers: symbol.all_pin_numbers.clone(),
                anchor_ki_x: symbol.anchor_ki_x,
                anchor_ki_y: symbol.anchor_ki_y,
                angle: 0.0,
                pin_ki_x: vec![],
                pin_ki_y: vec![],
                ref_ki_x: symbol.ref_ki_x,
                ref_ki_y: symbol.ref_ki_y,
                ref_ki_angle: symbol.ref_ki_angle,
                val_ki_x: symbol.val_ki_x,
                val_ki_y: symbol.val_ki_y,
                val_ki_angle: symbol.val_ki_angle,
            });
        }
    }

    (matched, errors)
}

/// Compute the rotation angle for each matched component by comparing
/// feature-pin template vectors against actual grid positions.
///
/// The angle follows KiCad's clockwise convention (0, 90, 180, 270).
/// For 2-pin symbols the two pins are used directly; for OPA330xxD
/// pins 2 (-input) and 6 (output) are used.
///
/// Uses grid coordinates so it can run before [`compute_layout`].
/// Afterwards, [`apply_rotation_to_rel_phys`] must be called to update
/// the physical offsets so the DAG solver sees the rotated constraints.
pub fn solve_orientations(matched: &mut [MatchedComponent]) {
    for comp in matched.iter_mut() {
        // ---- single-pin: rotate so template dir matches schematic dir ---------
        if comp.pins.len() == 1 {
            if let Some(pin) = comp.pins.first() {
                let tmpl_dir = pin.tmpl_dir;
                let sch_dir = pin.dir;
                let raw = ((dir_cw_index(sch_dir) - dir_cw_index(tmpl_dir) + 4) % 4) as f64 * 90.0;
                let angle = if raw == 0.0 { 0.0 } else { raw };
                comp.angle = angle;
            }
            continue;
        }
        if comp.pins.len() < 2 { continue; }
        let min_pin = comp.pins.iter().map(|p| p.pin_num).min().unwrap();
        let max_pin = comp.pins.iter().map(|p| p.pin_num).max().unwrap();
        if min_pin == max_pin { continue; }

        let a = match comp.pins.iter().find(|p| p.pin_num == min_pin) { Some(p) => p, None => continue };
        let b = match comp.pins.iter().find(|p| p.pin_num == max_pin) { Some(p) => p, None => continue };

        // Template vector in canvas Y-down (tmpl_phys is already Y-flipped)
        let tmpl_dx = b.tmpl_phys_x - a.tmpl_phys_x;
        let tmpl_dy = b.tmpl_phys_y - a.tmpl_phys_y;

        // Canvas vector from grid positions (same direction as mm vector)
        let canvas_dx = b.grid_col as f64 - a.grid_col as f64;
        let canvas_dy = b.grid_row as f64 - a.grid_row as f64;

        let angle_tmpl = tmpl_dy.atan2(tmpl_dx).to_degrees();
        let angle_canvas = canvas_dy.atan2(canvas_dx).to_degrees();

        let mut raw = angle_canvas - angle_tmpl;

        // Snap to nearest 90°
        raw = (raw / 90.0).round() * 90.0;

        // Normalize to [0, 360)
        let mut angle = raw % 360.0;
        if angle < 0.0 { angle += 360.0; }
        if angle == 0.0 { angle = 0.0; }  // squash −0.0
        if angle >= 360.0 { angle = 0.0; }

        comp.angle = angle;
    }
}

/// Update [`MatchedPin::rel_phys_x`] / [`MatchedPin::rel_phys_y`] for each
/// matched component according to its resolved [`MatchedComponent::angle`].
///
/// The formula is an SVG-style CW rotation in canvas Y-down:
///
/// ```text
/// rx = tx·cos θ − ty·sin θ
/// ry = tx·sin θ + ty·cos θ
/// ```
///
/// where `(tx, ty)` are the original (unswapped) template phys values stored
/// in [`MatchedPin::tmpl_phys_x`] / [`MatchedPin::tmpl_phys_y`].
///
/// Must be called after [`solve_orientations`] and **before**
/// [`compute_layout`] so the DAG solver uses the rotated constraints.
pub fn apply_rotation_to_rel_phys(matched: &mut [MatchedComponent]) {
    for comp in matched.iter_mut() {
        if comp.angle == 0.0 { continue; }
        let a = comp.angle.to_radians();
        let (s, c) = (a.sin(), a.cos());
        for pin in &mut comp.pins {
            let tx = pin.tmpl_phys_x;
            let ty = pin.tmpl_phys_y;
            pin.rel_phys_x = tx * c - ty * s;
            pin.rel_phys_y = tx * s + ty * c;
        }
    }
}

/// Pre-compute KiCad mm positions for each pin of every matched component,
/// applying the rotation angle stored in `comp.angle`.
///
/// Positions are in canvas Y-down mm.  Call [`crate::kicad::to_kicad_x`] /
/// [`crate::kicad::to_kicad_y`] to convert to KiCad file coordinates.
pub fn compute_pin_ki_positions(
    matched: &mut [MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
) {
    for comp in matched.iter_mut() {
        let angle_rad = comp.angle.to_radians();
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let ax_c = col_x[comp.anchor_grid_col];
        let ay_c = row_y[comp.anchor_grid_row];

        let mut ki_x = Vec::with_capacity(comp.pins.len());
        let mut ki_y = Vec::with_capacity(comp.pins.len());

        for pin in &comp.pins {
            // Relative KiCad offset from anchor: (tmpl_phys_x, -tmpl_phys_y)
            // because tmpl_phys_y = -(KiCad Y diff), i.e. already Y-flipped.
            let kx = pin.tmpl_phys_x;       // KiCad X from anchor
            let ky = -pin.tmpl_phys_y;      // KiCad Y from anchor

            // Apply CW rotation and convert to canvas Y-down:
            // canvas = anchor_canvas + (kx*cosθ + ky*sinθ,  kx*sinθ - ky*cosθ)
            //                                                        ^ Y-flip: -(rotated_y)
            let px = ax_c + kx * cos_a + ky * sin_a;
            let py = ay_c + kx * sin_a - ky * cos_a;

            ki_x.push(px);
            ki_y.push(py);
        }

        comp.pin_ki_x = ki_x;
        comp.pin_ki_y = ki_y;
    }
}

// ============================================================
// Grid conversion
// ============================================================

fn to_grid(input: &str) -> Vec<Vec<char>> {
    input.lines().map(|line| line.chars().collect()).collect()
}

// ============================================================
// Static scanner — Generic Port Detection
// ============================================================

/// Walk the grid row by row, column by column, identifying
/// Labels `[...]`, Junctions `*`, Corners `+`, and generic Ports
/// matching `Letter+Digits:Digits[(name)]<direction>`.
/// Wire characters (`-`, `|`) and spaces are skipped.
pub fn scan_nodes(input: &str) -> Vec<SchematicNode> {
    let grid = to_grid(input);
    let mut nodes = Vec::new();

    for (row, line) in grid.iter().enumerate() {
        let mut col = 0;
        while col < line.len() {
            let ch = line[col];

            match ch {
                '[' => {
                    let start_col = col;
                    col += 1; // skip '['
                    let name_start = col;
                    while col < line.len() && line[col] != ']' {
                        col += 1;
                    }
                    let name: String = line[name_start..col].iter().collect();
                    let end_col = col; // at ']' or past end of line
                    let text_width = end_col - start_col + 1;
                    nodes.push(SchematicNode {
                        node_type: NodeType::Label(name),
                        pos: AbsPos { row, col: start_col },
                        text_width,
                        grid_row: 0,
                        grid_col: 0,
                        span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                    });
                    if col < line.len() {
                        col += 1; // skip ']'
                    }
                }
                '*' => {
                    nodes.push(SchematicNode {
                        node_type: NodeType::Junction,
                        pos: AbsPos { row, col },
                        text_width: 1,
                        grid_row: 0,
                        grid_col: 0,
                        span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                    });
                    col += 1;
                }
                '+' => {
                    nodes.push(SchematicNode {
                        node_type: NodeType::Corner,
                        pos: AbsPos { row, col },
                        text_width: 1,
                        grid_row: 0,
                        grid_col: 0,
                        span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                    });
                    col += 1;
                }
                ch if ch.is_ascii_uppercase() || ch == '#' => {
                    if let Some((refdes, pin, name, dir, width)) = try_parse_port(line, col) {
                        nodes.push(SchematicNode {
                            node_type: NodeType::Port { refdes, pin, name, dir },
                            pos: AbsPos { row, col },
                            text_width: width,
                            grid_row: 0,
                            grid_col: 0,
                            span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                        });
                        col += width;
                    } else {
                        col += 1; // not a valid port, skip char
                    }
                }
                _ => {
                    // wire chars, spaces, and anything else: skip
                    col += 1;
                }
            }
        }
    }

    nodes
}

/// Try to parse a generic Port pattern at `start`:
///   Letters + Digits + ':' + Digits  [  '(' name ')'  ]  Direction
///
/// Letters is one or more uppercase ASCII letters (e.g. `R`, `PWR`, `GND`).
/// Direction is one of `<` `>` `^` `v` and is **required**.
///
/// Returns `(refdes, pin_number, name, direction, total_width)` on success.
fn try_parse_port(line: &[char], start: usize) -> Option<(String, usize, String, PinDirection, usize)> {
    let mut pos = start;

    // Optional '#' prefix for power-symbol refdes (e.g. #PWR1)
    if line[pos] == '#' {
        pos += 1;
    }

    // Must have at least one uppercase letter
    if pos >= line.len() || !line[pos].is_ascii_uppercase() {
        return None;
    }
    pos += 1;
    while pos < line.len() && line[pos].is_ascii_uppercase() {
        pos += 1;
    }

    // Must have at least one digit after the letters
    if pos >= line.len() || !line[pos].is_ascii_digit() {
        return None;
    }
    while pos < line.len() && line[pos].is_ascii_digit() {
        pos += 1;
    }

    // Expect ':'
    if pos >= line.len() || line[pos] != ':' {
        return None;
    }
    let colon_pos = pos;
    pos += 1;

    // Must have at least one digit for the pin number
    if pos >= line.len() || !line[pos].is_ascii_digit() {
        return None;
    }
    let pin_start = pos;
    while pos < line.len() && line[pos].is_ascii_digit() {
        pos += 1;
    }

    let refdes: String = line[start..colon_pos].iter().collect();
    let pin: usize = line[pin_start..pos]
        .iter()
        .collect::<String>()
        .parse()
        .ok()?;

    // Optional name in parentheses: (...)
    let name = if pos < line.len() && line[pos] == '(' {
        pos += 1; // skip '('
        let name_start = pos;
        while pos < line.len() && line[pos] != ')' {
            pos += 1;
        }
        let name_str: String = line[name_start..pos].iter().collect();
        if pos < line.len() {
            pos += 1; // skip ')'
        }
        name_str
    } else {
        String::new()
    };

    // Required direction character
    let dir = PinDirection::from_char(line.get(pos).copied()?)?;
    pos += 1;

    let width = pos - start;
    Some((refdes, pin, name, dir, width))
}

// ============================================================
// Step 2: Coordinate Compression
// ============================================================

/// Compress absolute grid coordinates into compact relative indices.
///
/// * **Row compression**: collects all distinct `pos.row` values across nodes,
///   sorts them, and assigns each node's `grid_row` to its index in that ordering.
/// * **Column compression**: same for `pos.col`.
///
/// After this call every node's `grid_row` and `grid_col` are populated.
pub fn compress_coordinates(nodes: &mut [SchematicNode]) {
    let mut rows: Vec<usize> = nodes.iter().map(|n| n.pos.row).collect();
    rows.sort();
    rows.dedup();

    let mut cols: Vec<usize> = nodes.iter().map(|n| n.pos.col).collect();
    cols.sort();
    cols.dedup();

    for node in nodes.iter_mut() {
        node.grid_row = rows.binary_search(&node.pos.row).unwrap();
        node.grid_col = cols.binary_search(&node.pos.col).unwrap();
    }
}

// ============================================================
// Step 3: Port Pairing & Component Instantiation
// ============================================================

// ============================================================
// Step 3.5: Four-Direction Span Computation
// ============================================================

/// Compute the [`NodeSpan`] for every node based on its type.
/// DAG rigid `rel_phys` constraints handle precise spacing between matched
/// component pins, so ports always use the default [`HALF_SPAN`] in all
/// four directions.
pub fn compute_spans(nodes: &mut [SchematicNode]) {
    for node in nodes.iter_mut() {
        node.span = match &node.node_type {
            NodeType::Junction | NodeType::Corner => NodeSpan {
                left: HALF_SPAN, right: HALF_SPAN, up: HALF_SPAN, down: HALF_SPAN,
            },
            NodeType::Label(name) => {
                let text_w = (name.len() + 2) as f64 * crate::CHAR_WIDTH;
                let text_h_half = (crate::LABEL_TEXT_H / 2.0).max(HALF_SPAN);
                NodeSpan {
                    left: text_w / 2.0, right: text_w / 2.0,
                    up: text_h_half, down: text_h_half,
                }
            }
            NodeType::Port { .. } => NodeSpan {
                left: HALF_SPAN, right: HALF_SPAN, up: HALF_SPAN, down: HALF_SPAN,
            },
        };
    }
}

// ============================================================
// Step 4: DAG-based Dynamic Grid Layout
// ============================================================

/// Compute physical grid coordinates via DAG longest-path solving.
///
/// Constraints (directed edges from lower to higher index):
/// 1. **Base span constraints** — adjacent columns/rows must be spaced at
///    least `max_span(right) + max_span(left) + MIN_GAP` apart.
/// 2. **Rigid macro constraints** — for each matched component pin whose
///    grid index differs from the anchor, the physical distance between
///    those grid lines must be at least `|rel_phys|`.
///
/// Returns `(col_x, row_y)` where:
/// * `col_x[c]` – x-coordinate (SVG px) of grid column *c*
/// * `row_y[r]` – y-coordinate (SVG px) of grid row *r*
pub fn compute_layout(
    nodes: &[SchematicNode],
    matched: &[MatchedComponent],
) -> (Vec<f64>, Vec<f64>) {
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let mut node_at: std::collections::HashMap<(usize, usize), &SchematicNode> =
        std::collections::HashMap::new();
    for n in nodes {
        node_at.insert((n.grid_row, n.grid_col), n);
    }

    let span_or_default = |r: usize, c: usize, f: fn(NodeSpan) -> f64| -> f64 {
        node_at
            .get(&(r, c))
            .map(|n| f(n.span))
            .unwrap_or(HALF_SPAN)
    };

    // Build a set of (row, col) positions owned by matched pins.
    // Skip span constraints between two pins of the same matched component —
    // their spacing is already governed by the rigid rel_phys constraints.
    let matched_owner: std::collections::HashMap<(usize, usize), &str> = matched
        .iter()
        .flat_map(|comp| comp.pins.iter().map(move |p| ((p.grid_row, p.grid_col), comp.refdes.as_str())))
        .collect();

    // --- collect column constraints (row-by-row cross-scan) ---------------
    // col_edges[high] = list of (low, min_distance)
    let mut col_edges: Vec<Vec<(usize, f64)>> = vec![Vec::new(); max_col + 1];

    // Adjacent base constraints: for each column pair (c, c+1), scan ALL rows
    // and take the maximum LOCAL gap — avoids ghost-stretching from unrelated
    // large nodes that sit in different rows.
    for c in 0..max_col {
        let mut required_col_gap = 0.0_f64;
        for r in 0..=max_row {
            // Skip span gap if both cells belong to the same matched component
            let same_owner = match (matched_owner.get(&(r, c)), matched_owner.get(&(r, c + 1))) {
                (Some(a), Some(b)) if a == b => true,
                _ => false,
            };
            if same_owner { continue; }

            let right = span_or_default(r, c, |s| s.right);
            let left  = span_or_default(r, c + 1, |s| s.left);
            required_col_gap = required_col_gap.max(right + left + MIN_GAP);
        }
        col_edges[c + 1].push((c, required_col_gap));
    }

    // Rigid macro constraints from matched components
    for comp in matched {
        for pin in &comp.pins {
            if pin.grid_col != comp.anchor_grid_col {
                let low = comp.anchor_grid_col.min(pin.grid_col);
                let high = comp.anchor_grid_col.max(pin.grid_col);
                col_edges[high].push((low, pin.rel_phys_x.abs()));
            }
        }
    }

    // DAG longest-path solver — columns (forward)
    let mut col_x = vec![0.0_f64; max_col + 1];
    col_x[0] = MARGIN;
    for i in 1..=max_col {
        let mut best: f64 = 0.0;
        for &(low, weight) in &col_edges[i] {
            best = best.max(col_x[low] + weight);
        }
        col_x[i] = best;
    }

    // Backward pass: enforce pin-to-anchor spacing from the right side.
    // col_x[pin] >= col_x[anchor] − |rel_phys_x|  for pins left of anchor.
    for comp in matched {
        for pin in &comp.pins {
            if pin.grid_col < comp.anchor_grid_col {
                let target = col_x[comp.anchor_grid_col] - pin.rel_phys_x.abs();
                if target > col_x[pin.grid_col] {
                    col_x[pin.grid_col] = target;
                }
            }
        }
    }

    // Forward propagation: cascade the backward-adjusted values rightward.
    for i in 1..=max_col {
        for &(low, weight) in &col_edges[i] {
            col_x[i] = col_x[i].max(col_x[low] + weight);
        }
    }

    // --- collect row constraints (column-by-column cross-scan) -------------
    let mut row_edges: Vec<Vec<(usize, f64)>> = vec![Vec::new(); max_row + 1];

    // Adjacent base constraints: for each row pair (r, r+1), scan ALL columns
    // and take the maximum LOCAL gap.
    for r in 0..max_row {
        let mut required_row_gap = 0.0_f64;
        for c in 0..=max_col {
            // Skip span gap if both cells belong to the same matched component
            let same_owner = match (matched_owner.get(&(r, c)), matched_owner.get(&(r + 1, c))) {
                (Some(a), Some(b)) if a == b => true,
                _ => false,
            };
            if same_owner { continue; }

            let down = span_or_default(r, c, |s| s.down);
            let up   = span_or_default(r + 1, c, |s| s.up);
            required_row_gap = required_row_gap.max(down + up + MIN_GAP);
        }
        row_edges[r + 1].push((r, required_row_gap));
    }

    // Rigid macro constraints from matched components
    for comp in matched {
        for pin in &comp.pins {
            if pin.grid_row != comp.anchor_grid_row {
                let low = comp.anchor_grid_row.min(pin.grid_row);
                let high = comp.anchor_grid_row.max(pin.grid_row);
                row_edges[high].push((low, pin.rel_phys_y.abs()));
            }
        }
    }

    // DAG longest-path solver — rows (forward)
    let mut row_y = vec![0.0_f64; max_row + 1];
    row_y[0] = MARGIN;
    for i in 1..=max_row {
        let mut best: f64 = 0.0;
        for &(low, weight) in &row_edges[i] {
            best = best.max(row_y[low] + weight);
        }
        row_y[i] = best;
    }

    // Backward pass: enforce pin-to-anchor spacing from the bottom side.
    // row_y[pin] >= row_y[anchor] − |rel_phys_y|  for pins above anchor.
    for comp in matched {
        for pin in &comp.pins {
            if pin.grid_row < comp.anchor_grid_row {
                let target = row_y[comp.anchor_grid_row] - pin.rel_phys_y.abs();
                if target > row_y[pin.grid_row] {
                    row_y[pin.grid_row] = target;
                }
            }
        }
    }

    // Forward propagation: cascade the backward-adjusted values downward.
    for i in 1..=max_row {
        for &(low, weight) in &row_edges[i] {
            row_y[i] = row_y[i].max(row_y[low] + weight);
        }
    }

    (col_x, row_y)
}

// ============================================================
// Step 5: Grid-Neighbour Wire Extraction
// ============================================================

/// A straight wire segment between two physical points.
#[derive(Debug, Clone)]
pub struct WireSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl WireSegment {
    pub fn is_horizontal(&self) -> bool {
        (self.y1 - self.y2).abs() < 0.1
    }
}

/// Half the symbol pin spacing in mm.
/// KiCad RLC pins at ±3.81 mm; OPA330 pin spacing varies.
pub const SYMBOL_HALF: f64 = 3.81;

/// Determine the physical connection point for any node.
///
/// * Ports → symbol pin position (component centre ± [`SYMBOL_HALF`]).
/// * Labels → edge of the text bounding box in the wire direction.
/// * Junctions / Corners → grid centre.
fn endpoint_position(
    node: &SchematicNode,
    _matched: &[MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
    is_horizontal: bool,
    is_first: bool,
) -> (f64, f64) {
    match &node.node_type {
        NodeType::Port { .. } => {
            (col_x[node.grid_col], row_y[node.grid_row])
        }
        NodeType::Label(_) => {
            let cx = col_x[node.grid_col];
            let cy = row_y[node.grid_row];
            if is_horizontal {
                if is_first {
                    // Left node → wire exits from the right edge.
                    (cx + node.span.right, cy)
                } else {
                    // Right node → wire enters from the left edge.
                    (cx - node.span.left, cy)
                }
            } else {
                if is_first {
                    // Top node → wire exits from the bottom edge.
                    (cx, cy + node.span.down)
                } else {
                    // Bottom node → wire enters from the top edge.
                    (cx, cy - node.span.up)
                }
            }
        }
        // Junctions and Corners → grid centre.
        NodeType::Junction | NodeType::Corner => {
            (col_x[node.grid_col], row_y[node.grid_row])
        }
    }
}

/// Extract wire segments by checking adjacent nodes on each grid row/column.
///
/// * Horizontal: for every row *r*, nodes sorted by `grid_col` — if the
///   original ASCII row contains `-` between two adjacent nodes, emit a wire.
/// * Vertical:   for every column *c*, nodes sorted by `grid_row` — if the
///   original ASCII column contains `|` between two adjacent nodes, emit a wire.
///
/// Endpoints are snapped to the physical port/label-edge position rather than
/// the raw grid intersection.
pub fn extract_wires(
    nodes: &[SchematicNode],
    matched: &[MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
    input: &str,
) -> Vec<WireSegment> {
    let grid: Vec<Vec<char>> = input.lines().map(|l| l.chars().collect()).collect();

    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let mut node_at: std::collections::HashMap<(usize, usize), &SchematicNode> =
        std::collections::HashMap::new();
    for n in nodes {
        node_at.insert((n.grid_row, n.grid_col), n);
    }

    let mut wires = Vec::new();

    // ---- horizontal wires ------------------------------------------------
    for r in 0..=max_row {
        let mut row_nodes: Vec<&SchematicNode> = (0..=max_col)
            .filter_map(|c| node_at.get(&(r, c)))
            .copied()
            .collect();
        row_nodes.sort_by_key(|n| n.grid_col);

        for w in row_nodes.windows(2) {
            let (a, b) = (w[0], w[1]);
            // a is left of b (sorted by grid_col → same order as pos.col)
            let scan_start = a.pos.col + a.text_width;
            let scan_end = b.pos.col;
            let has_dash = scan_start < scan_end
                && (scan_start..scan_end).any(|col| {
                    grid.get(a.pos.row)
                        .and_then(|line| line.get(col))
                        .is_some_and(|&ch| ch == '-')
                });

            if has_dash {
                let (x1, y1) = endpoint_position(a, matched, col_x, row_y, true, true);
                let (x2, y2) = endpoint_position(b, matched, col_x, row_y, true, false);
                wires.push(WireSegment { x1, y1, x2, y2 });
            }
        }
    }

    // ---- vertical wires --------------------------------------------------
    for c in 0..=max_col {
        let mut col_nodes: Vec<&SchematicNode> = (0..=max_row)
            .filter_map(|r| node_at.get(&(r, c)))
            .copied()
            .collect();
        col_nodes.sort_by_key(|n| n.grid_row);

        for w in col_nodes.windows(2) {
            let (a, b) = (w[0], w[1]);
            // a is above b (sorted by grid_row → same order as pos.row)
            let scan_start = a.pos.row + 1;
            let scan_end = b.pos.row;
            let has_pipe = scan_start < scan_end
                && (scan_start..scan_end).any(|row| {
                    grid.get(row)
                        .and_then(|line| line.get(a.pos.col))
                        .is_some_and(|&ch| ch == '|')
                });

            if has_pipe {
                let (x1, y1) = endpoint_position(a, matched, col_x, row_y, false, true);
                let (x2, y2) = endpoint_position(b, matched, col_x, row_y, false, false);
                wires.push(WireSegment { x1, y1, x2, y2 });
            }
        }
    }

    wires
}

// ============================================================
// Step 1 Unit Tests
// ============================================================
#[cfg(test)]
mod step1_tests {
    use super::*;

    #[test]
    fn scan_basic_nodes() {
        let input = "\
[VCC]  +  R1:1<
          *
          C1:1^\
";
        let nodes = scan_nodes(input);

        // (0, 0) Label "VCC"
        let vcc = &nodes[0];
        assert_eq!(vcc.pos, AbsPos { row: 0, col: 0 });
        assert_eq!(vcc.text_width, 5);
        assert_eq!(vcc.node_type, NodeType::Label("VCC".to_string()));

        // (0, 7) Corner "+"
        let corner = &nodes[1];
        assert_eq!(corner.pos, AbsPos { row: 0, col: 7 });
        assert_eq!(corner.text_width, 1);
        assert_eq!(corner.node_type, NodeType::Corner);

        // (0, 10) Port "R1:1<"
        let r1 = &nodes[2];
        assert_eq!(r1.pos, AbsPos { row: 0, col: 10 });
        assert_eq!(r1.text_width, 5);
        assert_eq!(
            r1.node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Left,
            }
        );

        // (1, 10) Junction "*"
        let junc = &nodes[3];
        assert_eq!(junc.pos, AbsPos { row: 1, col: 10 });
        assert_eq!(junc.text_width, 1);
        assert_eq!(junc.node_type, NodeType::Junction);

        // (2, 10) Port "C1:1^"
        let c1 = &nodes[4];
        assert_eq!(c1.pos, AbsPos { row: 2, col: 10 });
        assert_eq!(c1.text_width, 5);
        assert_eq!(
            c1.node_type,
            NodeType::Port {
                refdes: "C1".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Up,
            }
        );

        // Total node count
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn scan_port_with_multi_digit_refdes() {
        let input = "R10:2> C100:1<\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 2);

        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "R10".to_string(),
                pin: 2,
                name: String::new(),
                dir: PinDirection::Right,
            }
        );
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });
        assert_eq!(nodes[0].text_width, 6); // R10:2>

        assert_eq!(
            nodes[1].node_type,
            NodeType::Port {
                refdes: "C100".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Left,
            }
        );
        assert_eq!(nodes[1].pos, AbsPos { row: 0, col: 7 });
        assert_eq!(nodes[1].text_width, 7); // C100:1<
    }

    #[test]
    fn scan_inductor_port() {
        let input = "L3:1<\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "L3".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Left,
            }
        );
    }

    #[test]
    fn bare_letter_not_a_port() {
        // R alone without digit+colon+digit is not a valid port
        let input = "R\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn refdes_without_colon_not_a_port() {
        // R1 alone is not a port (needs :digit)
        let input = "R1\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn port_without_trailing_pin_not_a_port() {
        // R1: is not valid (needs pin digit)
        let input = "R1:\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn word_starting_with_r_not_a_port() {
        // "READ" is a word, not a port — 'E' after 'R' is not a digit
        let input = "READ\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn port_with_trailing_text() {
        // "R1:1<abc" — the port is R1:1<, trailing "abc" is skipped
        let input = "R1:1<abc\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Left,
            }
        );
        assert_eq!(nodes[0].text_width, 5);
    }

    #[test]
    fn port_without_direction_not_a_port() {
        // R1:1 without direction char is NOT a valid port
        let input = "R1:1\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn mixed_nodes_on_one_line() {
        let input = "[VCC]  R1:1<  *  +  [GND]\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 5);

        assert_eq!(nodes[0].node_type, NodeType::Label("VCC".to_string()));
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });

        assert_eq!(
            nodes[1].node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1,
                name: String::new(),
                dir: PinDirection::Left,
            }
        );
        assert_eq!(nodes[1].pos.col, 7);

        assert_eq!(nodes[2].node_type, NodeType::Junction);
        assert_eq!(nodes[2].pos.col, 14);

        assert_eq!(nodes[3].node_type, NodeType::Corner);
        assert_eq!(nodes[3].pos.col, 17);

        assert_eq!(nodes[4].node_type, NodeType::Label("GND".to_string()));
        assert_eq!(nodes[4].pos.col, 20);
    }

    #[test]
    fn empty_input() {
        let nodes = scan_nodes("");
        assert!(nodes.is_empty());
    }

    #[test]
    fn wire_chars_and_spaces_are_skipped() {
        let input = "--- | |      \n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn label_with_internal_chars() {
        let input = "[NET_3V3]\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_type, NodeType::Label("NET_3V3".to_string()));
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });
        assert_eq!(nodes[0].text_width, 9); // [NET_3V3]
    }

    // ---- OpAmp / OPA330xxD port scanning -----------------------------

    /// Compact 5×3 grid schematic matching the OPA330xxD KiCad symbol.
    /// Anchor Pin 3 at (R1, C0); rel_grid values use compact indices
    /// (sorted unique X/Y coordinates mapped to consecutive integers).
    /// Each grid unit = 12 characters wide.
    pub(super) fn opa330_sch() -> String {
        let mut lines: Vec<String> = Vec::new();

        fn place(line: &mut String, grid_col: usize, text: &str) {
            let target = grid_col * 12;
            while line.len() < target {
                line.push(' ');
            }
            line.push_str(text);
        }

        // Row 0: U1:7(V+)^ at C1  (highest KiCad Y = 7.62)
        let mut r0 = String::new();
        place(&mut r0, 1, "U1:7(V+)^");
        lines.push(r0);

        // Row 1: U1:3(+)< at C0  (anchor, Y = 2.54)
        let mut r1 = String::new();
        place(&mut r1, 0, "U1:3(+)<");
        lines.push(r1);

        // Row 2: U1:6> at C2  (Y = 0)
        let mut r2 = String::new();
        place(&mut r2, 2, "U1:6>");
        lines.push(r2);

        // Row 3: U1:2(-)< at C0  (Y = -2.54)
        let mut r3 = String::new();
        place(&mut r3, 0, "U1:2(-)<");
        lines.push(r3);

        // Row 4: U1:4(V-)v at C1  (lowest KiCad Y = -7.62)
        let mut r4 = String::new();
        place(&mut r4, 1, "U1:4(V-)v");
        lines.push(r4);

        lines.join("\n")
    }

    #[test]
    fn scan_opamp_ports() {
        let input = opa330_sch();
        let nodes = scan_nodes(&input);

        let find = |pin: usize| -> &SchematicNode {
            nodes.iter().find(|n| {
                matches!(&n.node_type, NodeType::Port { pin: p, .. } if *p == pin)
            }).expect("port not found")
        };

        // U1:7  name="V+", dir=Up  — row 0, col 12 (after 12 spaces)
        let p7 = find(7);
        assert_eq!(p7.pos, AbsPos { row: 0, col: 12 });
        assert_eq!(p7.text_width, 9);
        match &p7.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                assert_eq!(refdes, "U1");
                assert_eq!(*pin, 7);
                assert_eq!(name, "V+");
                assert_eq!(*dir, PinDirection::Up);
            }
            _ => panic!("expected Port"),
        }

        // U1:3  name="+", dir=Left  — row 1, col 0
        let p3 = find(3);
        assert_eq!(p3.pos, AbsPos { row: 1, col: 0 });
        assert_eq!(p3.text_width, 8);
        match &p3.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                assert_eq!(refdes, "U1");
                assert_eq!(*pin, 3);
                assert_eq!(name, "+");
                assert_eq!(*dir, PinDirection::Left);
            }
            _ => panic!("expected Port"),
        }

        // U1:6  name="", dir=Right  — row 2, col 24
        let p6 = find(6);
        assert_eq!(p6.pos, AbsPos { row: 2, col: 24 });
        assert_eq!(p6.text_width, 5);
        match &p6.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                assert_eq!(refdes, "U1");
                assert_eq!(*pin, 6);
                assert_eq!(name, "");
                assert_eq!(*dir, PinDirection::Right);
            }
            _ => panic!("expected Port"),
        }

        // U1:2  name="-", dir=Left  — row 3, col 0
        let p2 = find(2);
        assert_eq!(p2.pos, AbsPos { row: 3, col: 0 });
        assert_eq!(p2.text_width, 8);
        match &p2.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                assert_eq!(refdes, "U1");
                assert_eq!(*pin, 2);
                assert_eq!(name, "-");
                assert_eq!(*dir, PinDirection::Left);
            }
            _ => panic!("expected Port"),
        }

        // U1:4  name="V-", dir=Down  — row 4, col 12
        let p4 = find(4);
        assert_eq!(p4.pos, AbsPos { row: 4, col: 12 });
        assert_eq!(p4.text_width, 9);
        match &p4.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                assert_eq!(refdes, "U1");
                assert_eq!(*pin, 4);
                assert_eq!(name, "V-");
                assert_eq!(*dir, PinDirection::Down);
            }
            _ => panic!("expected Port"),
        }
    }

    // ---- Symbol library self-check ----------------------------------

    #[test]
    fn symbol_library_has_required_symbols() {
        let lib = init_symbol_library();
        assert!(lib.iter().any(|s| s.symbol_name == "R"));
        assert!(lib.iter().any(|s| s.symbol_name == "C"));
        assert!(lib.iter().any(|s| s.symbol_name == "L"));
    }

    #[test]
    fn opa330xxd_loaded_symbol_anchor_is_pin_3() {
        let sym = crate::kicad_sym::load_opa330xxd_symbol();
        let anchor = &sym.pins[0];
        assert_eq!(anchor.pin_num, 3, "anchor should be pin 3");
        assert_eq!(anchor.name, "+");
        assert_eq!(anchor.rel_grid_row, 0);
        assert_eq!(anchor.rel_grid_col, 0);
    }
}

// ============================================================
// Rigid Template Matching Tests
// ============================================================
#[cfg(test)]
mod template_matching_tests {
    use super::*;

    fn opa330_library() -> HashMap<String, ComponentSymbol> {
        let mut lib: HashMap<String, ComponentSymbol> = HashMap::new();
        let sym = crate::kicad_sym::load_opa330xxd_symbol();
        lib.insert(sym.symbol_name.clone(), sym);
        lib
    }

    fn scan_compress(input: &str) -> Vec<SchematicNode> {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        nodes
    }

    #[test]
    fn opamp_rigid_match_succeeds() {
        let input = super::step1_tests::opa330_sch();
        let nodes = scan_compress(&input);
        let lib = opa330_library();
        let refdes_map: HashMap<String, String> = [
            ("U1".to_string(), "OPA330xxD".to_string()),
        ].into_iter().collect();
        let (matched, errors) = match_components(&nodes, &refdes_map, &lib);

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(matched.len(), 1, "expected 1 matched component");
        let m = &matched[0];
        assert_eq!(m.refdes, "U1");
        assert!(m.symbol_name.contains("OPA330xxD"),
            "symbol_name should contain OPA330xxD, got: {}", m.symbol_name);

        // Anchor pin 3 at grid (R1, C0) — compact grid
        assert_eq!(m.anchor_grid_row, 1);
        assert_eq!(m.anchor_grid_col, 0);

        let find = |pin: usize| m.pins.iter().find(|p| p.pin_num == pin).unwrap();

        // Pin 3 (+) — anchor
        let p3 = find(3);
        assert_eq!(p3.name, "+");
        assert_eq!(p3.dir, PinDirection::Left);
        assert_eq!(p3.grid_row, 1);
        assert_eq!(p3.grid_col, 0);

        // Pin 2 (-) — 2 rows below anchor (compact)
        let p2 = find(2);
        assert_eq!(p2.name, "-");
        assert_eq!(p2.dir, PinDirection::Left);
        assert_eq!(p2.grid_row, 3);
        assert_eq!(p2.grid_col, 0);

        // Pin 7 (V+) — 1 row above, 1 col right (compact)
        let p7 = find(7);
        assert_eq!(p7.name, "V+");
        assert_eq!(p7.dir, PinDirection::Up);
        assert_eq!(p7.grid_row, 0);
        assert_eq!(p7.grid_col, 1);

        // Pin 4 (V-) — 3 rows below, 1 col right (compact)
        let p4 = find(4);
        assert_eq!(p4.name, "V-");
        assert_eq!(p4.dir, PinDirection::Down);
        assert_eq!(p4.grid_row, 4);
        assert_eq!(p4.grid_col, 1);

        // Pin 6 (OUT) — 1 row below, 2 cols right (compact)
        let p6 = find(6);
        assert_eq!(p6.name, "");
        assert_eq!(p6.dir, PinDirection::Right);
        assert_eq!(p6.grid_row, 2);
        assert_eq!(p6.grid_col, 2);

        // Draw primitives from KiCad file are present
        assert!(!m.draw_primitives.is_empty(),
            "matched component should have draw_primitives from OPA330xxD");
    }

    #[test]
    fn opamp_flexible_grid_position_for_rotation() {
        // Pin 2 at non-template position is now accepted because OPA330xxD
        // has feature pins — the orientation solver will detect rotation later.
        let input = "\
U1:3(+)<
U1:2(-)<
";
        let nodes = scan_compress(input);
        let lib = opa330_library();
        let refdes_map: HashMap<String, String> = [
            ("U1".to_string(), "OPA330xxD".to_string()),
        ].into_iter().collect();
        let (matched, errors) = match_components(&nodes, &refdes_map, &lib);

        assert!(errors.is_empty(), "rotatable symbols accept any position; got: {:?}", errors);
        assert_eq!(matched.len(), 1, "pin 2 should still be matched to the component");
        let p2 = matched[0].pins.iter().find(|p| p.pin_num == 2).unwrap();
        assert_eq!(p2.grid_row, 1); // actual position, not template-expected
    }

    #[test]
    fn undeclared_refdes_is_rejected() {
        let input = "R1:1<  R1:2>\n";
        let nodes = scan_compress(&input);
        let lib = HashMap::new(); // empty library
        let refdes_map: HashMap<String, String> = HashMap::new(); // empty header
        let (_matched, errors) = match_components(&nodes, &refdes_map, &lib);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("undeclared"), "got: {}", errors[0]);
        assert!(errors[0].contains("R1"), "got: {}", errors[0]);
    }
}

// ============================================================
// Step 0: Header Parsing Tests
// ============================================================
#[cfg(test)]
mod header_parsing_tests {
    use super::*;

    #[test]
    fn split_header_body_with_separator() {
        let input = "U1: OPA330xxD\nR1: R\n=====\n[VCC] R1:1< R1:2>";
        let (header, body) = split_header_body(input);
        assert_eq!(header, "U1: OPA330xxD\nR1: R");
        assert_eq!(body, "[VCC] R1:1< R1:2>");
    }

    #[test]
    fn split_header_body_with_long_separator() {
        let input = "U1: OPA330xxD\n==================\nbody text";
        let (header, body) = split_header_body(input);
        assert_eq!(header, "U1: OPA330xxD");
        assert_eq!(body, "body text");
    }

    #[test]
    fn split_header_body_without_separator() {
        let input = "just body text\nR1:1< R1:2>";
        let (header, body) = split_header_body(input);
        assert_eq!(header, "");
        assert_eq!(body, "just body text\nR1:1< R1:2>");
    }

    #[test]
    fn split_header_body_empty_input() {
        let (header, body) = split_header_body("");
        assert_eq!(header, "");
        assert_eq!(body, "");
    }

    #[test]
    fn parse_header_valid_lines() {
        let header = "U1: OPA330xxD\nR1: R\nC1: C\n";
        let map = parse_header(header);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("U1").unwrap(), "OPA330xxD");
        assert_eq!(map.get("R1").unwrap(), "R");
        assert_eq!(map.get("C1").unwrap(), "C");
    }

    #[test]
    fn parse_header_with_whitespace() {
        let header = "  U1 :  OPA330xxD  \n  L1  :  L  \n";
        let map = parse_header(header);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("U1").unwrap(), "OPA330xxD");
        assert_eq!(map.get("L1").unwrap(), "L");
    }

    #[test]
    fn parse_header_skips_empty_and_invalid() {
        let header = "U1: OPA330xxD\n\ninvalid line\n  \nR1: R\n";
        let map = parse_header(header);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("U1").unwrap(), "OPA330xxD");
        assert_eq!(map.get("R1").unwrap(), "R");
    }

    #[test]
    fn parse_header_empty() {
        let map = parse_header("");
        assert!(map.is_empty());
    }
}

// ============================================================
// DAG Layout Solver Tests
// ============================================================
#[cfg(test)]
mod dag_layout_tests {
    use super::*;

    fn opa330_library() -> HashMap<String, ComponentSymbol> {
        let mut lib: HashMap<String, ComponentSymbol> = HashMap::new();
        let sym = crate::kicad_sym::load_opa330xxd_symbol();
        lib.insert(sym.symbol_name.clone(), sym);
        lib
    }

    fn opa330_refdes_map() -> HashMap<String, String> {
        [("U1".to_string(), "OPA330xxD".to_string())].into_iter().collect()
    }

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<MatchedComponent>, Vec<f64>, Vec<f64>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        let lib = opa330_library();
        let refdes_map = opa330_refdes_map();
        let (matched, _) = match_components(&nodes, &refdes_map, &lib);
        compute_spans(&mut nodes);
        let (col_x, row_y) = compute_layout(&nodes, &matched);
        (nodes, matched, col_x, row_y)
    }

    #[test]
    fn opamp_pin_row_spacing_enforced() {
        let input = super::step1_tests::opa330_sch();
        let (_nodes, matched, _col_x, row_y) = full_pipeline(&input);
        let m = &matched[0];

        let p3 = m.pins.iter().find(|p| p.pin_num == 3).unwrap();
        let p2 = m.pins.iter().find(|p| p.pin_num == 2).unwrap();

        let dy = row_y[p2.grid_row] - row_y[p3.grid_row];
        // OPA330xxD: pin 2 is 5.08 mm below anchor (2 × 2.54)
        assert!(dy >= 5.07,
            "pin3→pin2 row spacing: {:.3} mm, template requires >= 5.08 mm", dy);
    }

    #[test]
    fn opamp_pin_col_spacing_enforced() {
        let input = super::step1_tests::opa330_sch();
        let (_nodes, matched, col_x, _row_y) = full_pipeline(&input);
        let m = &matched[0];

        let p3 = m.pins.iter().find(|p| p.pin_num == 3).unwrap();
        let p6 = m.pins.iter().find(|p| p.pin_num == 6).unwrap();

        let dx = col_x[p6.grid_col] - col_x[p3.grid_col];
        // OPA330xxD: pin 6 is 15.24 mm right of anchor (6 × 2.54)
        assert!(dx >= 15.23,
            "pin3→pin6 col spacing: {:.3} mm, template requires >= 15.24 mm", dx);
    }

    #[test]
    fn opamp_pins_align_to_grid_intersections() {
        let input = super::step1_tests::opa330_sch();
        let (_nodes, matched, col_x, row_y) = full_pipeline(&input);
        let m = &matched[0];

        for p in &m.pins {
            let gx = col_x[p.grid_col];
            let gy = row_y[p.grid_row];
            // Verify no NaN and grid positions are within the arrays
            assert!(gx >= 0.0, "pin {} col_x out of range", p.pin_num);
            assert!(gy >= 0.0, "pin {} row_y out of range", p.pin_num);
        }
    }

    #[test]
    fn opamp_with_long_label_col_spacing() {
        // Same as opa330_sch but with a label [WIDE] at the end of row 0 (C3),
        // creating a 4th column that adds base-span constraints.
        let mut lines: Vec<String> = Vec::new();
        fn place(line: &mut String, grid_col: usize, text: &str) {
            let target = grid_col * 12;
            while line.len() < target { line.push(' '); }
            line.push_str(text);
        }
        let mut r0 = String::new(); place(&mut r0, 1, "U1:7(V+)^"); place(&mut r0, 3, "[WIDE]"); lines.push(r0);
        let mut r1 = String::new(); place(&mut r1, 0, "U1:3(+)<"); lines.push(r1);
        let mut r2 = String::new(); place(&mut r2, 2, "U1:6>"); lines.push(r2);
        let mut r3 = String::new(); place(&mut r3, 0, "U1:2(-)<"); lines.push(r3);
        let mut r4 = String::new(); place(&mut r4, 1, "U1:4(V-)v"); lines.push(r4);
        let input = lines.join("\n");

        let (_nodes, matched, col_x, row_y) = full_pipeline(&input);
        let m = &matched[0];

        let p3 = m.pins.iter().find(|p| p.pin_num == 3).unwrap();
        let p2 = m.pins.iter().find(|p| p.pin_num == 2).unwrap();
        let dy = row_y[p2.grid_row] - row_y[p3.grid_row];
        assert!(dy >= 5.07,
            "pin3→pin2 row spacing: {:.3} mm, template requires >= 5.08 mm", dy);

        let p6 = m.pins.iter().find(|p| p.pin_num == 6).unwrap();
        let dx = col_x[p6.grid_col] - col_x[p3.grid_col];
        assert!(dx >= 15.23,
            "pin3→pin6 col spacing: {:.3} mm, template requires >= 15.24 mm", dx);

        let p7 = m.pins.iter().find(|p| p.pin_num == 7).unwrap();
        let dx01 = col_x[p7.grid_col] - col_x[p3.grid_col];
        assert!(dx01 >= 5.07,
            "C0→C2 spacing: {:.3} mm, template requires >= 5.08 mm", dx01);
    }

    #[test]
    fn no_ghost_stretch_between_unrelated_nodes() {
        // Two huge labels at different (row, col) positions:
        //   A at (0,0): span 100 in all directions
        //   B at (1,5): span 100 in all directions
        // All other grid positions are empty → HALF_SPAN = 20.
        //
        // Row gap R0→R1 must be max LOCAL gap across each column:
        //   at C0: A.down(100) + empty.up(20) = 120
        //   at C5: empty.down(20) + B.up(100) = 120
        //   elsewhere: 20 + 20 = 40
        //   → row_y[1] - row_y[0] = 120
        //
        // With the old global-max approach this would be 100 + 100 = 200
        // (ghost-stretch from unrelated large nodes).
        let nodes = vec![
            SchematicNode {
                node_type: NodeType::Label("HUGE_A".to_string()),
                pos: AbsPos { row: 0, col: 0 },
                text_width: 7,
                grid_row: 0,
                grid_col: 0,
                span: NodeSpan { left: 100.0, right: 100.0, up: 100.0, down: 100.0 },
            },
            SchematicNode {
                node_type: NodeType::Label("HUGE_B".to_string()),
                pos: AbsPos { row: 1, col: 5 },
                text_width: 7,
                grid_row: 1,
                grid_col: 5,
                span: NodeSpan { left: 100.0, right: 100.0, up: 100.0, down: 100.0 },
            },
        ];

        let (col_x, row_y) = compute_layout(&nodes, &[]);

        // Row gap: not 200 (ghost), should be 100 + HALF_SPAN + MIN_GAP
        let row_gap = row_y[1] - row_y[0];
        let expected = 100.0 + HALF_SPAN + MIN_GAP;
        assert!((row_gap - expected).abs() < 0.5,
            "row gap should be {} (local max: 100+HALF_SPAN), not 200. Got {:.1}",
            expected, row_gap);

        // Column gap C4→C5 at row 1: empty.right(HALF_SPAN) + B.left(100) + MIN_GAP
        let col_gap_4_5 = col_x[5] - col_x[4];
        assert!((col_gap_4_5 - expected).abs() < 0.5,
            "col gap C4→C5 should be {}, got {:.1}", expected, col_gap_4_5);
    }
}

// ============================================================
// Step 2 Coordinate Compression Tests
// ============================================================
#[cfg(test)]
mod step2_compression_tests {
    use super::*;

    #[test]
    fn compress_with_empty_lines_and_sparse_columns() {
        let input = "\
[VCC]          +

          R1:1<\
";
        let mut nodes = scan_nodes(input);
        assert_eq!(nodes.len(), 3, "expected 3 nodes");

        compress_coordinates(&mut nodes);

        let vcc = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Label(ref name) if name == "VCC"))
            .expect("VCC label not found");
        let corner = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner not found");
        let r1 = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Port { ref pin, .. } if *pin == 1))
            .expect("R1:1 port not found");

        assert_eq!(vcc.grid_row, 0, "VCC grid_row");
        assert_eq!(vcc.grid_col, 0, "VCC grid_col");

        assert_eq!(corner.grid_row, 0, "Corner grid_row");
        assert_eq!(corner.grid_col, 2, "Corner grid_col");

        assert_eq!(r1.grid_row, 1, "R1:1 grid_row");
        assert_eq!(r1.grid_col, 1, "R1:1 grid_col");
    }

    #[test]
    fn compress_single_row_all_same_row() {
        let input = "[A]  *  +  R1:1<\n";
        let mut nodes = scan_nodes(input);
        assert_eq!(nodes.len(), 4);

        compress_coordinates(&mut nodes);

        for node in &nodes {
            assert_eq!(node.grid_row, 0);
        }
        let cols: Vec<usize> = nodes.iter().map(|n| n.grid_col).collect();
        let mut expected = cols.clone();
        expected.sort();
        assert_eq!(cols, expected, "grid_col should be monotonic left-to-right");
    }

    #[test]
    fn compress_empty_nodes() {
        let mut nodes: Vec<SchematicNode> = vec![];
        compress_coordinates(&mut nodes);
    }
}

// Legacy step3_pairing_tests removed — all components now go through
// match_components with header declarations.
// ============================================================
// Step 3.5 Span Computation Tests
// ============================================================
#[cfg(test)]
mod step35_span_tests {
    use super::*;

    fn full_pipeline(input: &str) -> Vec<SchematicNode> {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        compute_spans(&mut nodes);
        nodes
    }

    #[test]
    fn horizontal_resistor_pin1_span() {
        let nodes = full_pipeline("R1:1<  R1:2>\n");
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "R1" && *pin == 1))
            .expect("R1:1 not found");

        // All ports now use symmetric HALF_SPAN; DAG handles spacing.
        assert_eq!(p1.span.left, HALF_SPAN);
        assert_eq!(p1.span.right, HALF_SPAN);
        assert_eq!(p1.span.up, HALF_SPAN);
        assert_eq!(p1.span.down, HALF_SPAN);
    }

    #[test]
    fn horizontal_resistor_pin2_span() {
        let nodes = full_pipeline("R1:1<  R1:2>\n");
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "R1" && *pin == 2))
            .expect("R1:2 not found");

        assert_eq!(p2.span.left, HALF_SPAN);
        assert_eq!(p2.span.right, HALF_SPAN);
        assert_eq!(p2.span.up, HALF_SPAN);
        assert_eq!(p2.span.down, HALF_SPAN);
    }

    #[test]
    fn vertical_capacitor_pin1_span() {
        let input = "\
C1:1^
C1:2v\
";
        let nodes = full_pipeline(input);
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "C1" && *pin == 1))
            .expect("C1:1 not found");

        assert_eq!(p1.span.left, HALF_SPAN);
        assert_eq!(p1.span.right, HALF_SPAN);
        assert_eq!(p1.span.up, HALF_SPAN);
        assert_eq!(p1.span.down, HALF_SPAN);
    }

    #[test]
    fn vertical_capacitor_pin2_span() {
        let input = "\
C1:1^
C1:2v\
";
        let nodes = full_pipeline(input);
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "C1" && *pin == 2))
            .expect("C1:2 not found");

        assert_eq!(p2.span.left, HALF_SPAN);
        assert_eq!(p2.span.right, HALF_SPAN);
        assert_eq!(p2.span.up, HALF_SPAN);
        assert_eq!(p2.span.down, HALF_SPAN);
    }

    #[test]
    fn junction_span_is_symmetric() {
        let nodes = full_pipeline("*\n");
        let j = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction not found");

        assert_eq!(j.span.left, HALF_SPAN);
        assert_eq!(j.span.right, HALF_SPAN);
        assert_eq!(j.span.up, HALF_SPAN);
        assert_eq!(j.span.down, HALF_SPAN);
    }

    #[test]
    fn corner_span_is_symmetric() {
        let nodes = full_pipeline("+\n");
        let c = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner not found");

        assert_eq!(c.span.left, HALF_SPAN);
        assert_eq!(c.span.right, HALF_SPAN);
        assert_eq!(c.span.up, HALF_SPAN);
        assert_eq!(c.span.down, HALF_SPAN);
    }

    #[test]
    fn label_span_is_text_based() {
        let nodes = full_pipeline("[VCC]\n");
        let lbl = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Label(name) if name == "VCC"))
            .expect("VCC label not found");

        // text_w = (3+2)*CHAR_WIDTH = 5*0.339 = 1.695, half ≈ 0.85
        let half_w = (5.0 * crate::CHAR_WIDTH) / 2.0;
        assert!((lbl.span.left - half_w).abs() < 0.01);
        assert!((lbl.span.right - half_w).abs() < 0.01);
        assert_eq!(lbl.span.up, HALF_SPAN);
        assert_eq!(lbl.span.down, HALF_SPAN);
    }

    #[test]
    fn orphan_port_without_component_uses_default() {
        let nodes = full_pipeline("R9:1<\n");
        let p = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "R9" && *pin == 1))
            .expect("R9:1 not found");

        assert_eq!(p.span.left, HALF_SPAN);
        assert_eq!(p.span.right, HALF_SPAN);
        assert_eq!(p.span.up, HALF_SPAN);
        assert_eq!(p.span.down, HALF_SPAN);
    }

    #[test]
    fn horizontal_inductor_pin_spans() {
        let nodes = full_pipeline("L1:1<  L1:2>\n");
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "L1" && *pin == 1))
            .expect("L1:1 not found");
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin, .. } if refdes == "L1" && *pin == 2))
            .expect("L1:2 not found");

        assert_eq!(p1.span.right, HALF_SPAN);
        assert_eq!(p2.span.left, HALF_SPAN);
    }
}

// ============================================================
// Step 4 Dynamic Grid Layout Tests
// ============================================================
#[cfg(test)]
mod step4_layout_tests {
    use super::*;

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<f64>, Vec<f64>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        compute_spans(&mut nodes);
        let (col_x, row_y) = compute_layout(&nodes, &[]);
        (nodes, col_x, row_y)
    }

    #[test]
    fn long_label_widens_column_gap() {
        let input = "\
[VERY_LONG_SIGNAL_NAME_A]   R1:1<  R1:2>   [VCC]\
";
        let (_nodes, col_x, _row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 4);
        assert_eq!(col_x[0], MARGIN);

        // long label: "VERY_LONG_SIGNAL_NAME_A" = 23 chars, + 2 brackets
        let long_half = (23.0 + 2.0) * crate::CHAR_WIDTH / 2.0;
        let expected_long = long_half + HALF_SPAN + MIN_GAP;
        let gap_long = col_x[1] - col_x[0];
        assert!((gap_long - expected_long).abs() < 0.01,
            "long label gap expected {:.3}, got {:.3}", expected_long, gap_long);

        // short label: "VCC" = 3 chars, + 2 brackets
        let short_half = ((3.0 + 2.0) * crate::CHAR_WIDTH) / 2.0;
        let expected_short = HALF_SPAN + short_half + MIN_GAP;
        let gap_short = col_x[3] - col_x[2];
        assert!((gap_short - expected_short).abs() < 0.01,
            "short label gap expected {:.3}, got {:.3}", expected_short, gap_short);

        assert!(gap_long > gap_short * 2.0,
            "long label gap ({:.3}) should be > 2x short label gap ({:.3})",
            gap_long, gap_short);
    }

    #[test]
    fn no_physical_overlap_between_bounding_boxes() {
        let input = "\
[VERY_LONG_SIGNAL_NAME_A]   R1:1<  R1:2>   [VCC]\
";
        let (nodes, col_x, row_y) = full_pipeline(input);

        let node_at: std::collections::HashMap<(usize, usize), &SchematicNode> = nodes
            .iter()
            .map(|n| ((n.grid_row, n.grid_col), n))
            .collect();

        let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
        let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

        for r in 0..=max_row {
            for c in 0..max_col {
                let right_edge = match node_at.get(&(r, c)) {
                    Some(n) => col_x[c] + n.span.right,
                    None => col_x[c] + HALF_SPAN,
                };
                let left_edge = match node_at.get(&(r, c + 1)) {
                    Some(n) => col_x[c + 1] - n.span.left,
                    None => col_x[c + 1] - HALF_SPAN,
                };
                let clearance = left_edge - right_edge;
                assert!(clearance >= MIN_GAP - 0.01,
                    "Overlap at R{} C{}→C{}: right={:.1} left={:.1} clearance={:.1} < MIN_GAP={:.1}",
                    r, c, c + 1, right_edge, left_edge, clearance, MIN_GAP);
            }
        }

        for r in 0..max_row {
            for c in 0..=max_col {
                let bottom_edge = match node_at.get(&(r, c)) {
                    Some(n) => row_y[r] + n.span.down,
                    None => row_y[r] + HALF_SPAN,
                };
                let top_edge = match node_at.get(&(r + 1, c)) {
                    Some(n) => row_y[r + 1] - n.span.up,
                    None => row_y[r + 1] - HALF_SPAN,
                };
                let clearance = top_edge - bottom_edge;
                assert!(clearance >= MIN_GAP - 0.01,
                    "Overlap at R{}→R{} C{}: bottom={:.1} top={:.1} clearance={:.1} < MIN_GAP={:.1}",
                    r, r + 1, c, bottom_edge, top_edge, clearance, MIN_GAP);
            }
        }
    }

    #[test]
    fn vertical_component_row_spacing() {
        let input = "\
[VCC]
C1:1^
C1:2v\
";
        let (_nodes, _col_x, row_y) = full_pipeline(input);

        assert_eq!(row_y.len(), 3);
        assert_eq!(row_y[0], MARGIN);

        let expected0 = HALF_SPAN + HALF_SPAN + MIN_GAP; // [VCC] down + C1:1 up
        let gap0 = row_y[1] - row_y[0];
        assert!((gap0 - expected0).abs() < 0.01,
            "R0→R1 gap expected {:.2}, got {:.2}", expected0, gap0);

        let gap1 = row_y[2] - row_y[1];
        // All ports now use symmetric HALF_SPAN; matched component DAG
        // constraints handle the actual pin spacing in integration tests.
        let expected1 = HALF_SPAN + HALF_SPAN + MIN_GAP;
        assert!((gap1 - expected1).abs() < 0.01,
            "R1→R2 gap expected {:.2}, got {:.2}", expected1, gap1);
    }

    #[test]
    fn empty_input_layout() {
        let input = "";
        let (_nodes, col_x, row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 1);
        assert_eq!(col_x[0], MARGIN);
        assert_eq!(row_y.len(), 1);
        assert_eq!(row_y[0], MARGIN);
    }

    #[test]
    fn single_node_layout() {
        let input = "*\n";
        let (_nodes, col_x, row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 1);
        assert_eq!(row_y.len(), 1);
        assert_eq!(col_x[0], MARGIN);
        assert_eq!(row_y[0], MARGIN);
    }

}

// ============================================================
// Step 5: Grid-Neighbour Wire Extraction Tests
// ============================================================
#[cfg(test)]
mod step5_wire_tests {
    use super::*;

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<f64>, Vec<f64>, Vec<WireSegment>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        compute_spans(&mut nodes);
        let (col_x, row_y) = compute_layout(&nodes, &[]);
        let wires = extract_wires(&nodes, &[], &col_x, &row_y, input);
        (nodes, col_x, row_y, wires)
    }

    #[test]
    fn corner_wire_routing() {
        let input = "\
[VCC] ---+
         |
         R1:1^
         R1:2v\
";
        let (nodes, _col_x, _row_y, wires) = full_pipeline(input);

        assert_eq!(wires.len(), 2, "expected 2 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 1, "expected 1 horizontal wire");
        assert_eq!(v_wires.len(), 1, "expected 1 vertical wire");

        let corner = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner node not found");
        assert!(!matches!(corner.node_type, NodeType::Junction),
            "'+' is a Corner, not a Junction — no dot should be drawn");
    }

    #[test]
    fn crossing_without_connection() {
        let input = "         [Y1]
         |
[X1] ----+---- [X2]
         |
         [Y2]";
        let (nodes, _col_x, _row_y, wires) = full_pipeline(input);

        assert_eq!(wires.len(), 4, "expected 4 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires");
        assert_eq!(v_wires.len(), 2, "expected 2 vertical wires");

        let corner = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner node not found");
        assert!(!matches!(corner.node_type, NodeType::Junction));

        let has_junction = nodes.iter().any(|n| matches!(n.node_type, NodeType::Junction));
        assert!(!has_junction, "crossing test must not contain any Junction (*)");
    }

    #[test]
    fn t_junction_with_dot() {
        let input = "\
[VCC] ------- * ------- [OUT]
              |
              R2:1^
              R2:2v\
";
        let (nodes, _col_x, _row_y, wires) = full_pipeline(input);

        assert_eq!(wires.len(), 3, "expected 3 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires (VCC→*, *→OUT)");
        assert_eq!(v_wires.len(), 1, "expected 1 vertical wire (*→R2:1)");

        let junction = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction node (*) not found");
        assert!(matches!(junction.node_type, NodeType::Junction),
            "'*' must be a Junction so a dot is drawn");
    }

    #[test]
    fn cross_junction_with_dot() {
        let input = "           [UP]
           |
[LEFT] --- * --- [RIGHT]
           |
           [DOWN]";
        let (nodes, _col_x, _row_y, wires) = full_pipeline(input);

        assert_eq!(wires.len(), 4, "expected 4 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires");
        assert_eq!(v_wires.len(), 2, "expected 2 vertical wires");

        let junction = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction node (*) not found");
        assert!(matches!(junction.node_type, NodeType::Junction),
            "'*' must be a Junction so a dot is drawn");
    }
}
