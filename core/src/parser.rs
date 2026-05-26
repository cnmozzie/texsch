// ============================================================
// Step 1: Node Identification & Absolute Coordinate Extraction
// Port-based Grid Architecture — Anchor-based Multi-Port Symbols
// ============================================================

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

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
// Text-Grid Template System (Step 0: Template Definitions)
// ============================================================

/// Column reference for a grid assertion.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ColRef {
    /// Fixed offset from anchor start column.
    At(i32),
    /// Fixed offset, but guaranteed to be at least `text_width` columns
    /// to the right of the anchor (so the arrow never overlaps the refdes text).
    AtRight(i32),
}

impl ColRef {
    fn resolve(&self, anchor_col: usize, text_width: usize) -> usize {
        match self {
            ColRef::At(off) => (anchor_col as i32 + off) as usize,
            ColRef::AtRight(off) => {
                let pos = (anchor_col as i32 + off) as usize;
                pos.max(anchor_col + text_width)
            }
        }
    }
}

/// Rotate a `PinDirection` 90° clockwise.
/// Left→Up, Up→Right, Right→Down, Down→Left.
fn rotate_dir_cw(dir: PinDirection) -> PinDirection {
    match dir {
        PinDirection::Left => PinDirection::Up,
        PinDirection::Up => PinDirection::Right,
        PinDirection::Right => PinDirection::Down,
        PinDirection::Down => PinDirection::Left,
    }
}

/// One pin assertion in a text-grid template orientation.
#[derive(Debug, Clone)]
pub(crate) struct GridAssertion {
    pub(crate) pin_num: usize,
    /// Row offset from the refdes anchor text start.
    pub(crate) delta_row: i32,
    /// Column reference (fixed or text-width-relative).
    pub(crate) col_ref: ColRef,
    /// Expected arrow character at this position.
    pub(crate) expected_dir: PinDirection,
}

/// One orientation variant of a text-grid template.
#[derive(Debug, Clone)]
pub(crate) struct OrientationVariant {
    /// KiCad CW rotation angle (0, 90, 180, 270).
    pub(crate) angle: f64,
    /// Pin assertions for this orientation.
    pub(crate) assertions: Vec<GridAssertion>,
}

/// Text-grid template for a component symbol.
/// Defines how arrow characters surround the refdes text for each orientation.
#[derive(Debug, Clone)]
pub(crate) struct TextGridTemplate {
    pub(crate) orientations: Vec<OrientationVariant>,
}

/// Build a text-grid template from a [`ComponentSymbol`].
///
/// **All** positions (refdes + every pin) are derived from KiCad physical mm
/// coordinates via the same formula:
///
/// ```text
///   abs_ki_x = anchor_ki_x + pin.rel_phys_x      (mm from symbol origin)
///   abs_ki_y = anchor_ki_y - pin.rel_phys_y      (mm, undo Y-flip)
///   grid_col = round(abs_ki_x / 2.54)            (grid units)
///   grid_row = round(-abs_ki_y / 2.54)           (Y-down grid units)
/// ```
///
/// The refdes sits at KiCad centre (0,0) → grid (0,0).  Every pin arrow
/// is asserted at its computed (grid_row, grid_col) relative to the refdes.
///
/// Remaining 3 orientations generated by 90° CW rotation:
///   (dr', dc') = (dc, -dr)    dir' = rotate_dir_cw(dir)
pub(crate) fn build_text_template(sym: &ComponentSymbol) -> TextGridTemplate {
    // 1. Collect absolute KiCad mm positions for every pin + refdes at (0,0).
    struct Pt { pin_num: usize, x: f64, y: f64, dir: PinDirection }
    let mut pts: Vec<Pt> = sym.pins.iter().map(|p| {
        Pt {
            pin_num: p.pin_num,
            x: sym.anchor_ki_x + p.rel_phys_x,
            y: sym.anchor_ki_y - p.rel_phys_y,   // undo Y-flip
            dir: p.dir,
        }
    }).collect();

    // 2. Compact-grid ranks: unique X ascending, unique Y descending.
    let mut xs: Vec<f64> = pts.iter().map(|p| p.x).collect();
    xs.push(0.0); // refdes centre
    xs.sort_by(|a,b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a,b| (*a - *b).abs() < 0.001);

    let mut ys: Vec<f64> = pts.iter().map(|p| p.y).collect();
    ys.push(0.0); // refdes centre
    ys.sort_by(|a,b| b.partial_cmp(a).unwrap()); // descending = top row first
    ys.dedup_by(|a,b| (*a - *b).abs() < 0.001);

    let x_rank = |x: f64| xs.iter().position(|&v| (v - x).abs() < 0.001).unwrap() as i32;
    let y_rank = |y: f64| ys.iter().position(|&v| (v - y).abs() < 0.001).unwrap() as i32;

    let refdes_cx = x_rank(0.0);
    let refdes_cy = y_rank(0.0);

    let mut base = Vec::new();

    for pt in &pts {
        let mut grid_col = x_rank(pt.x) - refdes_cx;
        let mut grid_row = y_rank(pt.y) - refdes_cy;

        // Pin at refdes origin → push one unit outward.
        if grid_row == 0 && grid_col == 0 {
            match pt.dir {
                PinDirection::Up    => { grid_row = -1; }
                PinDirection::Down  => { grid_row = 1; }
                PinDirection::Left  => { grid_col = -1; }
                PinDirection::Right => { grid_col = 1; }
            }
        }

        let right_side = grid_col >= 0 && pt.dir == PinDirection::Right;
        let cr = if right_side {
            ColRef::AtRight(grid_col.abs())
        } else {
            ColRef::At(grid_col)
        };

        base.push(GridAssertion {
            pin_num: pt.pin_num,
            delta_row: grid_row,
            col_ref: cr,
            expected_dir: pt.dir,
        });
    }

    let mut orientations = Vec::new();

    for angle_idx in 0..4 {
        let angle = angle_idx as f64 * 90.0;
        let assertions: Vec<GridAssertion> = base.iter().map(|a| {
            let mut dr = a.delta_row;
            let mut dc = match a.col_ref {
                ColRef::At(v) | ColRef::AtRight(v) => v,
            };
            let mut ed = a.expected_dir;

            for _ in 0..angle_idx {
                let new_dr = dc;
                let new_dc = -dr;
                dr = new_dr;
                dc = new_dc;
                ed = rotate_dir_cw(ed);
            }

            // AtRight only makes sense when the pin faces Right after rotation.
            let cr = if ed == PinDirection::Right && dc >= 0 {
                ColRef::AtRight(dc)
            } else {
                ColRef::At(dc)
            };

            GridAssertion {
                pin_num: a.pin_num,
                delta_row: dr,
                col_ref: cr,
                expected_dir: ed,
            }
        }).collect();

        orientations.push(OrientationVariant { angle, assertions });
    }

    TextGridTemplate {
        orientations,
    }
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
    /// A component pin arrow (populated by template matching).
    Port { refdes: String, pin: usize, name: String, dir: PinDirection },
    /// A component refdes text anchor (populated by template matching).
    /// Sits at the KiCad symbol centre (0,0) in compact-grid space.
    Anchor { refdes: String },
    /// A net label, e.g. "[VCC]", "[GND]"
    Label(String),
    /// An electrical junction point marked by '*'
    Junction,
    /// A wire corner/crossing marked by '+'
    Corner,
    /// A spatial placeholder (`.`) that occupies a grid cell to prevent
    /// compact-grid row/column collapse, without generating any electrical
    /// connections or appearing in netlist / KiCad output.
    Placeholder,
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
    /// Temporary: absolute row of anchor pin before compression (internal use).
    pub(crate) _anchor_abs_row: usize,
    /// Temporary: absolute column of anchor pin before compression (internal use).
    pub(crate) _anchor_abs_col: usize,
    /// Reference property offset from symbol origin (KiCad Y-up, mm).
    pub ref_ki_x: f64,
    pub ref_ki_y: f64,
    pub ref_ki_angle: f64,
    /// Value property offset from symbol origin (KiCad Y-up, mm).
    pub val_ki_x: f64,
    pub val_ki_y: f64,
    pub val_ki_angle: f64,
}

/// Source-map entry mapping a component port text fragment to its location
/// in the original ASCII input.  Used by the frontend to highlight the
/// corresponding text when a schematic element is selected.
/// Distinguishes Port spans (delete whole component) from Label spans (delete single).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanKind {
    Port,
    Label,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ComponentTextSpan {
    /// Component reference designator (e.g. "U1", "R1") or label name (e.g. "[In1]").
    pub refdes: String,
    /// Whether this span is a Port or Label.
    pub kind: SpanKind,
    /// 1-based absolute line number in the full editor text.
    pub line_number: usize,
    /// 1-based starting column of the port text fragment.
    pub start_col: usize,
    /// 1-based ending column of the port text fragment (inclusive).
    pub end_col: usize,
}

/// Records a refdes that was auto-incremented to avoid collision with an
/// existing component instance.  The frontend uses this to update the editor
/// text (replace port refdes in body, add header declaration).
///
/// `positions` records the body-relative (0-based row, 0-based col, text_width)
/// of each port text that was reassigned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefdesReassignment {
    pub old_refdes: String,
    pub new_refdes: String,
    pub symbol_name: String,
    /// Body-relative positions of each reassigned port text:
    /// (0_based_row, 0_based_col, text_width).
    pub positions: Vec<(usize, usize, usize)>,
}

/// Split a refdes into its non-digit prefix and trailing numeric suffix.
/// E.g. "R1" → ("R", 1), "#GND12" → ("#GND", 12), "U10" → ("U", 10).
fn split_refdes(refdes: &str) -> (&str, u32) {
    let chars: Vec<char> = refdes.chars().collect();
    let mut digit_start = chars.len();
    for (i, c) in chars.iter().enumerate().rev() {
        if c.is_ascii_digit() {
            digit_start = i;
        } else {
            break;
        }
    }
    let prefix = &refdes[..digit_start];
    let num: u32 = refdes[digit_start..].parse().unwrap_or(0);
    (prefix, num)
}

/// Generate a unique refdes by incrementing the numeric suffix of `base`
/// until it doesn't collide with any existing header declaration, anchor group,
/// or already-assigned refdes from this batch.
fn generate_unique_refdes(
    base: &str,
    anchor_refdes: &std::collections::BTreeMap<String, Vec<&RefdesAnchor>>,
    refdes_to_symbol: &HashMap<String, String>,
    already_assigned: &std::collections::HashSet<String>,
    reserved_refdes: &std::collections::HashSet<String>,
) -> String {
    let (prefix, mut num) = split_refdes(base);
    num += 1;
    loop {
        let candidate = format!("{}{}", prefix, num);
        if !refdes_to_symbol.contains_key(&candidate)
            && !anchor_refdes.contains_key(&candidate)
            && !already_assigned.contains(&candidate)
            && !reserved_refdes.contains(&candidate)
        {
            return candidate;
        }
        num += 1;
    }
}

/// Split input into header and body sections separated by a line matching `^===+$`.
///
/// Returns `(header_str, body_str, body_line_offset)` where `body_line_offset`
/// is the 0-based absolute line index of the first body line in the full input.
/// If no separator is found, the entire input is treated as body and the header
/// is empty with offset 0.
pub fn split_header_body(input: &str) -> (&str, &str, usize) {
    for line in input.lines() {
        if line.len() >= 3 && line.chars().all(|c| c == '=') {
            let (header, body) = input.split_at(
                input.match_indices(line).next().unwrap().0
            );
            // body starts after the === line.
            // strip only leading blank lines — spaces are significant ASCII-art indentation.
            let body_start = body.find('\n').map(|p| p + 1).unwrap_or(body.len());
            let body_str = body[body_start..].trim_start_matches(|c: char| c == '\n' || c == '\r');

            // Compute how many lines precede body_str in the original input.
            let before_len = input.len() - body_str.len();
            let body_line_offset = input[..before_len].lines().count();

            return (header.trim_end(), body_str, body_line_offset);
        }
    }
    ("", input, 0)
}

/// Result of splitting input into three sections via up to two `====` separators.
pub struct SectionSplit {
    pub header: String,
    pub grid1_body: String,
    pub grid2_body: String,
    /// 0-based line index of Grid1's first body line in the full input.
    pub grid1_line_offset: usize,
    /// 0-based line index of Grid2's first body line in the full input.
    pub grid2_line_offset: usize,
    /// 0-based line index of the first separator, or 0 if none.
    pub sep1_line: usize,
    /// 0-based line index of the second separator, or 0 if none.
    pub sep2_line: usize,
}

/// Split input into three sections using up to two `====...` separator lines.
///
/// - 0 separators: entire input is Grid1 body, no header, no Grid2.
/// - 1 separator:  lines before → header, lines after → Grid1 body, no Grid2.
/// - 2+ separators: lines before first → header, between first and second → Grid1,
///   after second → Grid2.  Extra separators are treated as Grid2 content.
///
/// Body text is sliced directly from the original input, preserving exact
/// whitespace and line endings for accurate source-map column positions.
pub fn split_three_sections(input: &str) -> SectionSplit {
    // Find 0-based line indices of all `^===+$` lines.
    let sep_line_indices: Vec<usize> = input
        .lines()
        .enumerate()
        .filter(|(_, line)| line.len() >= 3 && line.chars().all(|c| c == '='))
        .map(|(i, _)| i)
        .collect();

    // Helper: byte offset of start of line N in `input`.
    fn line_byte_start(input: &str, target: usize) -> usize {
        let mut line_idx = 0;
        let mut byte_pos = 0;
        for (i, &b) in input.as_bytes().iter().enumerate() {
            if line_idx == target {
                return byte_pos;
            }
            if b == b'\n' {
                line_idx += 1;
                byte_pos = i + 1;
            }
        }
        byte_pos
    }

    // Helper: byte offset just past the end of line N (includes trailing \n if present).
    fn line_byte_end(input: &str, target: usize) -> usize {
        let mut line_idx = 0;
        for (i, &b) in input.as_bytes().iter().enumerate() {
            if b == b'\n' {
                if line_idx == target {
                    return i + 1; // include the newline
                }
                line_idx += 1;
            }
        }
        // Last line without trailing newline.
        if line_idx == target {
            return input.len();
        }
        input.len()
    }

    if sep_line_indices.is_empty() {
        return SectionSplit {
            header: String::new(),
            grid1_body: input.to_string(),
            grid2_body: String::new(),
            grid1_line_offset: 0,
            grid2_line_offset: 0,
            sep1_line: 0,
            sep2_line: 0,
        };
    }

    let sep1 = sep_line_indices[0];
    let sep1_start = line_byte_start(input, sep1);
    let header = input[..sep1_start].trim_end().to_string();
    let sep1_end = line_byte_end(input, sep1);

    if sep_line_indices.len() == 1 {
        let body_str = input[sep1_end..]
            .trim_start_matches(|c: char| c == '\n' || c == '\r');
        let body_line_offset = input[..sep1_end].lines().count();
        return SectionSplit {
            header,
            grid1_body: body_str.to_string(),
            grid2_body: String::new(),
            grid1_line_offset: body_line_offset,
            grid2_line_offset: 0,
            sep1_line: sep1,
            sep2_line: 0,
        };
    }

    let sep2 = sep_line_indices[1];
    let sep2_start = line_byte_start(input, sep2);
    let grid1_body = input[sep1_end..sep2_start]
        .trim_start_matches(|c: char| c == '\n' || c == '\r')
        .to_string();
    let grid1_line_offset = input[..sep1_end].lines().count();

    let sep2_end = line_byte_end(input, sep2);
    let grid2_body = input[sep2_end..]
        .trim_start_matches(|c: char| c == '\n' || c == '\r')
        .to_string();
    let grid2_line_offset = input[..sep2_end].lines().count();

    SectionSplit {
        header,
        grid1_body,
        grid2_body,
        grid1_line_offset,
        grid2_line_offset,
        sep1_line: sep1,
        sep2_line: sep2,
    }
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

/// Match refdes anchors against the symbol library using text-grid templates.
///
/// For each anchor, tries all 4 orientations (0°, 90°, 180°, 270°) in order.
/// For each orientation, checks that every grid assertion (arrow character at
/// the expected relative position) holds.  The first matching orientation wins
/// and locks the component angle.
///
/// Returns `(pin_nodes, matched_components, errors, reassignments)`.
/// `pin_nodes` are [`SchematicNode`]s of type [`NodeType::Port`] that represent
/// the matched pin arrow positions. They must be merged into the node list
/// before coordinate compression.
pub fn match_templates(
    anchors: &[RefdesAnchor],
    body: &str,
    refdes_to_symbol: &HashMap<String, String>,
    symbol_library: &HashMap<String, ComponentSymbol>,
    reserved_refdes: &std::collections::HashSet<String>,
) -> (Vec<SchematicNode>, Vec<MatchedComponent>, Vec<String>, Vec<RefdesReassignment>) {
    let grid: Vec<Vec<char>> = body.lines().map(|l| l.chars().collect()).collect();

    // Build text-grid templates for every symbol in the library.
    let templates: HashMap<String, TextGridTemplate> = symbol_library
        .iter()
        .map(|(name, sym)| (name.clone(), build_text_template(sym)))
        .collect();

    // Group anchors by refdes.
    let mut anchor_groups: std::collections::BTreeMap<String, Vec<&RefdesAnchor>> =
        std::collections::BTreeMap::new();
    for a in anchors {
        anchor_groups.entry(a.refdes.clone()).or_default().push(a);
    }

    let mut pin_nodes: Vec<SchematicNode> = Vec::new();
    let mut matched: Vec<MatchedComponent> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut reassignments: Vec<RefdesReassignment> = Vec::new();
    let mut assigned_new: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (refdes, group_anchors) in &anchor_groups {
        // Every refdes in the body MUST be declared in the header.
        let Some(symbol_name) = refdes_to_symbol.get(refdes) else {
            errors.push(format!(
                "{}: undeclared component refdes — add '{}: <SymbolName>' to the header before the === line",
                refdes, refdes
            ));
            continue;
        };

        let Some(symbol) = symbol_library.get(symbol_name) else {
            errors.push(format!(
                "{}: symbol \"{}\" not found in library",
                refdes, symbol_name
            ));
            continue;
        };

        let Some(template) = templates.get(symbol_name) else {
            errors.push(format!(
                "{}: no text-grid template for symbol \"{}\"",
                refdes, symbol_name
            ));
            continue;
        };

        let mut remaining: Vec<&RefdesAnchor> = group_anchors.to_vec();
        let mut instance_num = 0u32;

        loop {
            if remaining.is_empty() {
                break;
            }

            // Try each remaining anchor against all 4 orientations.
            let mut best_match: Option<(usize, usize, Vec<(usize, usize, &GridAssertion)>)> = None;

            for (ai, anchor) in remaining.iter().enumerate() {
                for (oi, orientation) in template.orientations.iter().enumerate() {
                    let mut hits: Vec<(usize, usize, &GridAssertion)> = Vec::new();
                    let mut all_ok = true;

                    for assertion in &orientation.assertions {
                        let cr = (anchor.row as i32 + assertion.delta_row) as usize;
                        let cc = assertion.col_ref.resolve(anchor.col, anchor.text_width);

                        if cr >= grid.len() {
                            all_ok = false;
                            break;
                        }
                        let line = &grid[cr];
                        if cc >= line.len() {
                            all_ok = false;
                            break;
                        }

                        if PinDirection::from_char(line[cc]) != Some(assertion.expected_dir) {
                            all_ok = false;
                            break;
                        }

                        hits.push((cr, cc, assertion));
                    }

                    if all_ok {
                        best_match = Some((ai, oi, hits));
                        break; // first matching orientation wins
                    }
                }
                if best_match.is_some() {
                    break;
                }
            }

            let Some((ai, oi, pin_matches)) = best_match else {
                // No orientation matched for any remaining anchor.
                // Report error only for the first anchor of this group.
                let a = remaining[0];
                errors.push(format!(
                    "{}: no matching orientation found at ({}, {}) — check that arrows (< > ^ v) surround the refdes correctly",
                    refdes, a.row, a.col
                ));
                break;
            };

            let anchor = remaining.remove(ai);

            // Determine refdes for this instance.
            // The first instance must also check reserved_refdes (cross-grid dedup).
            let instance_refdes: String = if instance_num == 0 && !reserved_refdes.contains(refdes.as_str()) {
                refdes.clone()
            } else {
                let new_refdes = generate_unique_refdes(
                    refdes, &anchor_groups, refdes_to_symbol, &assigned_new, reserved_refdes,
                );
                reassignments.push(RefdesReassignment {
                    old_refdes: refdes.clone(),
                    new_refdes: new_refdes.clone(),
                    symbol_name: symbol_name.clone(),
                    positions: vec![(anchor.row, anchor.col, anchor.text_width)],
                });
                assigned_new.insert(new_refdes.clone());
                new_refdes
            };

            // Create the refdes Anchor node — this becomes the component's grid anchor.
            pin_nodes.push(SchematicNode {
                node_type: NodeType::Anchor { refdes: instance_refdes.clone() },
                pos: AbsPos { row: anchor.row, col: anchor.col },
                text_width: anchor.text_width,
                grid_row: 0,
                grid_col: 0,
                span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
            });

            // Create pin nodes for each matched arrow.
            for &(row, col, assertion) in &pin_matches {
                pin_nodes.push(SchematicNode {
                    node_type: NodeType::Port {
                        refdes: instance_refdes.clone(),
                        pin: assertion.pin_num,
                        name: String::new(),
                        dir: assertion.expected_dir,
                    },
                    pos: AbsPos { row, col },
                    text_width: 1,
                    grid_row: 0,
                    grid_col: 0,
                    span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                });
            }

            // Record positions for reassignment (refdes text only, not arrows).
            if instance_num > 0 {
                if let Some(reass) = reassignments.last_mut() {
                    reass.positions = vec![(anchor.row, anchor.col, anchor.text_width)];
                }
            }

            // Build matched pins ordered by the symbol's pin list.
            let orientation = &template.orientations[oi];
            let angle = orientation.angle;

            let mut matched_pins: Vec<MatchedPin> = Vec::new();
            for tmpl_pin in &symbol.pins {
                if let Some((_row, _col, _assertion)) = pin_matches
                    .iter()
                    .find(|(_, _, a)| a.pin_num == tmpl_pin.pin_num)
                {
                    matched_pins.push(MatchedPin {
                        pin_num: tmpl_pin.pin_num,
                        name: tmpl_pin.name.clone(),
                        dir: tmpl_pin.dir,
                        grid_row: 0, // filled after compression
                        grid_col: 0,
                        rel_phys_x: tmpl_pin.rel_phys_x,
                        rel_phys_y: tmpl_pin.rel_phys_y,
                        tmpl_phys_x: tmpl_pin.rel_phys_x,
                        tmpl_phys_y: tmpl_pin.rel_phys_y,
                        tmpl_dir: tmpl_pin.dir,
                        pin_length_mm: tmpl_pin.pin_length_mm,
                    });
                }
            }

            if !matched_pins.is_empty() {
                matched.push(MatchedComponent {
                    refdes: instance_refdes,
                    symbol_name: symbol.symbol_name.clone(),
                    lib_id: symbol.lib_id.clone(),
                    pins: matched_pins,
                    anchor_grid_row: 0,
                    anchor_grid_col: 0,
                    // Temporary: store absolute pos of the refdes Anchor node.
                    _anchor_abs_row: anchor.row,
                    _anchor_abs_col: anchor.col,
                    draw_primitives: symbol.draw_primitives.clone(),
                    all_pin_numbers: symbol.all_pin_numbers.clone(),
                    anchor_ki_x: symbol.anchor_ki_x,
                    anchor_ki_y: symbol.anchor_ki_y,
                    angle,
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

            instance_num += 1;
        }
    }

    (pin_nodes, matched, errors, reassignments)
}

/// After coordinate compression, resolve each matched component's grid positions
/// by looking up its Anchor node (for component origin) and Port nodes (for pins).
pub fn resolve_matched_grid_positions(
    matched: &mut [MatchedComponent],
    nodes: &[SchematicNode],
) {
    // Build a map: (abs_row, abs_col) for Anchor nodes → compressed grid position.
    let anchor_lookup: HashMap<(usize, usize), (usize, usize)> = nodes
        .iter()
        .filter_map(|n| {
            if let NodeType::Anchor { .. } = &n.node_type {
                Some(((n.pos.row, n.pos.col), (n.grid_row, n.grid_col)))
            } else {
                None
            }
        })
        .collect();

    for comp in matched.iter_mut() {
        // Resolve the Anchor node's grid position → component origin.
        if let Some(&(gr, gc)) = anchor_lookup.get(&(comp._anchor_abs_row, comp._anchor_abs_col)) {
            comp.anchor_grid_row = gr;
            comp.anchor_grid_col = gc;
        }

        // Resolve each pin's grid position from Port nodes.
        for pin in &mut comp.pins {
            for node in nodes {
                if let NodeType::Port { refdes, pin: pn, .. } = &node.node_type {
                    if *refdes == comp.refdes && *pn == pin.pin_num {
                        pin.grid_row = node.grid_row;
                        pin.grid_col = node.grid_col;
                        break;
                    }
                }
            }
        }
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

/// Fill in `pin_ki_x` / `pin_ki_y` for every matched component using the
/// dynamic layout arrays. The pin's grid column/row already accounts for
/// any rotation (the ASCII stub is regenerated and re-compressed), so we
/// can use the grid position directly.
///
/// Positions are in canvas Y-down mm.  Call [`crate::kicad::to_kicad_x`] /
/// [`crate::kicad::to_kicad_y`] to convert to KiCad file coordinates.
pub fn compute_pin_ki_positions(
    matched: &mut [MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
) {
    for comp in matched.iter_mut() {
        comp.pin_ki_x = comp.pins.iter().map(|p| col_x[p.grid_col]).collect();
        comp.pin_ki_y = comp.pins.iter().map(|p| row_y[p.grid_row]).collect();
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
                    // Only treat as a label if a closing ']' exists
                    if col < line.len() {
                        let name: String = line[name_start..col].iter().collect();
                        let text_width = col - start_col + 1;
                        nodes.push(SchematicNode {
                            node_type: NodeType::Label(name),
                            pos: AbsPos { row, col: start_col },
                            text_width,
                            grid_row: 0,
                            grid_col: 0,
                            span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                        });
                        col += 1; // skip ']'
                    }
                    // No ']' → not a label; restart scan from start_col+1
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
                // Uppercase letters and '#' — refdes text is consumed by
                // find_refdes_anchors and template matching, not by scan_nodes.
                _ if ch.is_ascii_uppercase() || ch == '#' => {
                    col += 1;
                }
                '.' => {
                    nodes.push(SchematicNode {
                        node_type: NodeType::Placeholder,
                        pos: AbsPos { row, col },
                        text_width: 1,
                        grid_row: 0,
                        grid_col: 0,
                        span: NodeSpan { left: 0.0, right: 0.0, up: 0.0, down: 0.0 },
                    });
                    col += 1;
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

/// Build source-map entries for every Port, Label node, and refdes anchor
/// in the schematic body.
///
/// Each entry records the component refdes (or label name) and the 1-based absolute
/// line/column span of the text fragment in the original input.
/// A component's footprint includes both the refdes text anchor and all its
/// arrow characters — clicking or deleting any of them associates with the
/// entire component.
pub fn build_source_map(
    nodes: &[SchematicNode],
    anchors: &[RefdesAnchor],
    body_line_offset: usize,
) -> Vec<ComponentTextSpan> {
    let mut spans: Vec<ComponentTextSpan> = Vec::new();

    // Anchor nodes (refdes text)
    for n in nodes {
        if let NodeType::Anchor { refdes } = &n.node_type {
            spans.push(ComponentTextSpan {
                refdes: refdes.clone(),
                kind: SpanKind::Port,
                line_number: n.pos.row + body_line_offset + 1,
                start_col: n.pos.col + 1,
                end_col: n.pos.col + n.text_width,
            });
        }
    }

    // Port nodes (pin arrows)
    for n in nodes {
        if let NodeType::Port { refdes, .. } = &n.node_type {
            spans.push(ComponentTextSpan {
                refdes: refdes.clone(),
                kind: SpanKind::Port,
                line_number: n.pos.row + body_line_offset + 1,
                start_col: n.pos.col + 1,
                end_col: n.pos.col + n.text_width,
            });
        }
    }

    // Label nodes
    for n in nodes {
        if let NodeType::Label(name) = &n.node_type {
            spans.push(ComponentTextSpan {
                refdes: name.clone(),
                kind: SpanKind::Label,
                line_number: n.pos.row + body_line_offset + 1,
                start_col: n.pos.col + 1,
                end_col: n.pos.col + n.text_width,
            });
        }
    }

    // Refdes anchor text from find_refdes_anchors — only for anchors that
    // were NOT matched (no corresponding Anchor node already added).
    let anchor_refdes: std::collections::HashSet<&str> = nodes
        .iter()
        .filter_map(|n| {
            if let NodeType::Anchor { refdes } = &n.node_type {
                Some(refdes.as_str())
            } else { None }
        })
        .collect();

    for a in anchors {
        if !anchor_refdes.contains(a.refdes.as_str()) {
            spans.push(ComponentTextSpan {
                refdes: a.refdes.clone(),
                kind: SpanKind::Port,
                line_number: a.row + body_line_offset + 1,
                start_col: a.col + 1,
                end_col: a.col + a.text_width,
            });
        }
    }

    spans
}

/// A refdes text anchor found in the schematic body grid.
/// These are consumed by template matching to produce pin nodes and
/// matched components.
#[derive(Debug, Clone)]
pub struct RefdesAnchor {
    pub refdes: String,
    pub row: usize,
    pub col: usize,
    pub text_width: usize,
}

/// Try to parse a refdes string at `start`.
/// Pattern: optional '#', one or more uppercase letters, one or more digits.
/// E.g. "R1", "U10", "PWR3", "#GND2".
///
/// Returns `(refdes, width)` on success.
fn try_parse_refdes(line: &[char], start: usize) -> Option<(String, usize)> {
    let mut pos = start;

    // Optional '#' prefix for power-symbol refdes (e.g. #GND1)
    if pos < line.len() && line[pos] == '#' {
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

    // Must have at least one digit
    if pos >= line.len() || !line[pos].is_ascii_digit() {
        return None;
    }
    while pos < line.len() && line[pos].is_ascii_digit() {
        pos += 1;
    }

    let refdes: String = line[start..pos].iter().collect();
    let width = pos - start;
    Some((refdes, width))
}

/// Scan the body for refdes text anchors (e.g. "R1", "U1", "#GND2").
/// These are letters+digits patterns that identify component instances.
/// Each anchor's text position is used for template-based pin matching.
pub fn find_refdes_anchors(input: &str) -> Vec<RefdesAnchor> {
    let grid: Vec<Vec<char>> = input.lines().map(|l| l.chars().collect()).collect();
    let mut anchors = Vec::new();

    for (row, line) in grid.iter().enumerate() {
        let mut col = 0;
        while col < line.len() {
            let ch = line[col];
            if ch.is_ascii_uppercase() || ch == '#' {
                if let Some((refdes, width)) = try_parse_refdes(line, col) {
                    anchors.push(RefdesAnchor { refdes, row, col, text_width: width });
                    col += width;
                    continue;
                }
            }
            col += 1;
        }
    }

    anchors
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
            NodeType::Port { .. } | NodeType::Anchor { .. } => NodeSpan {
                left: HALF_SPAN, right: HALF_SPAN, up: HALF_SPAN, down: HALF_SPAN,
            },
            NodeType::Placeholder => NodeSpan {
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

    // Which columns / rows does each component have pins in?
    let comp_cols: std::collections::HashMap<&str, std::collections::HashSet<usize>> = {
        let mut m: std::collections::HashMap<&str, std::collections::HashSet<usize>> =
            std::collections::HashMap::new();
        for comp in matched {
            let cols: std::collections::HashSet<usize> =
                comp.pins.iter().map(|p| p.grid_col).collect();
            m.insert(comp.refdes.as_str(), cols);
        }
        m
    };
    let comp_rows: std::collections::HashMap<&str, std::collections::HashSet<usize>> = {
        let mut m: std::collections::HashMap<&str, std::collections::HashSet<usize>> =
            std::collections::HashMap::new();
        for comp in matched {
            let rows: std::collections::HashSet<usize> =
                comp.pins.iter().map(|p| p.grid_row).collect();
            m.insert(comp.refdes.as_str(), rows);
        }
        m
    };

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

            // Skip span gap if either cell belongs to a component that has pins
            // in BOTH columns — rigid rel_phys constraints handle the spacing.
            let spans_conflict_with_rigid = match (matched_owner.get(&(r, c)), matched_owner.get(&(r, c + 1))) {
                (Some(a), _) => comp_cols.get(a).map_or(false, |cols| cols.contains(&c) && cols.contains(&(c + 1))),
                (_, Some(b)) => comp_cols.get(b).map_or(false, |cols| cols.contains(&c) && cols.contains(&(c + 1))),
                _ => false,
            };
            if spans_conflict_with_rigid { continue; }

            let right = span_or_default(r, c, |s| s.right);
            let left  = span_or_default(r, c + 1, |s| s.left);
            required_col_gap = required_col_gap.max(right + left + MIN_GAP);
        }
        col_edges[c + 1].push((c, required_col_gap));
    }

    // Rigid macro constraints from matched components.
    // Reference = anchor PIN (pins[0]); weights = |rel_phys_x| in mm.
    // Using the anchor pin instead of the Anchor node avoids extra span
    // gaps from the refdes column being inserted between pin columns.
    for comp in matched {
        let a_col = comp.pins[0].grid_col;
        for pin in &comp.pins {
            if pin.grid_col != a_col {
                let low = a_col.min(pin.grid_col);
                let high = a_col.max(pin.grid_col);
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
    for comp in matched {
        let a_col = comp.pins[0].grid_col;
        for pin in &comp.pins {
            if pin.grid_col < a_col {
                let target = col_x[a_col] - pin.rel_phys_x.abs();
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

            // Skip span gap if either cell belongs to a component that has pins
            // in BOTH rows — rigid rel_phys constraints handle the spacing.
            let spans_conflict_with_rigid = match (matched_owner.get(&(r, c)), matched_owner.get(&(r + 1, c))) {
                (Some(a), _) => comp_rows.get(a).map_or(false, |rows| rows.contains(&r) && rows.contains(&(r + 1))),
                (_, Some(b)) => comp_rows.get(b).map_or(false, |rows| rows.contains(&r) && rows.contains(&(r + 1))),
                _ => false,
            };
            if spans_conflict_with_rigid { continue; }

            let down = span_or_default(r, c, |s| s.down);
            let up   = span_or_default(r + 1, c, |s| s.up);
            required_row_gap = required_row_gap.max(down + up + MIN_GAP);
        }
        row_edges[r + 1].push((r, required_row_gap));
    }

    // Rigid macro constraints from matched components
    for comp in matched {
        let a_row = comp.pins[0].grid_row;
        for pin in &comp.pins {
            if pin.grid_row != a_row {
                let low = a_row.min(pin.grid_row);
                let high = a_row.max(pin.grid_row);
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
    for comp in matched {
        let a_row = comp.pins[0].grid_row;
        for pin in &comp.pins {
            if pin.grid_row < a_row {
                let target = row_y[a_row] - pin.rel_phys_y.abs();
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
        // Junctions, Corners, Placeholders, and Anchors → grid centre.
        NodeType::Junction | NodeType::Corner | NodeType::Placeholder
        | NodeType::Anchor { .. } => {
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
        // Placeholder dots and Anchor text do not participate in wire extraction.
        if !matches!(n.node_type, NodeType::Placeholder | NodeType::Anchor { .. }) {
            node_at.insert((n.grid_row, n.grid_col), n);
        }
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
        // scan_nodes now only finds Labels, Junctions, Corners, Placeholders.
        // Refdes text and arrow characters are skipped (handled by find_refdes_anchors
        // and template matching).
        let input = "\
[VCC]  +  *
          .
          [GND]\
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

        // (0, 10) Junction "*"
        let junc = &nodes[2];
        assert_eq!(junc.pos, AbsPos { row: 0, col: 10 });
        assert_eq!(junc.node_type, NodeType::Junction);

        // (1, 10) Placeholder "."
        let dot = &nodes[3];
        assert_eq!(dot.pos, AbsPos { row: 1, col: 10 });
        assert_eq!(dot.node_type, NodeType::Placeholder);

        // (2, 10) Label "GND"
        let gnd = &nodes[4];
        assert_eq!(gnd.pos, AbsPos { row: 2, col: 10 });
        assert_eq!(gnd.node_type, NodeType::Label("GND".to_string()));

        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn find_refdes_anchors_basic() {
        let input = "<R1> <R10> #GND1\n";
        let anchors = find_refdes_anchors(input);
        assert_eq!(anchors.len(), 3);
        assert_eq!(anchors[0].refdes, "R1");
        assert_eq!(anchors[0].col, 1);
        assert_eq!(anchors[0].text_width, 2);
        assert_eq!(anchors[1].refdes, "R10");
        assert_eq!(anchors[1].text_width, 3);
        assert_eq!(anchors[2].refdes, "#GND1");
        assert_eq!(anchors[2].text_width, 5);
    }

    #[test]
    fn find_refdes_anchors_in_opamp_layout() {
        // OPA330 arrow-based template
        let input = "\
       ^
    <
U1          >
    <
       v\
";
        let anchors = find_refdes_anchors(input);
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].refdes, "U1");
    }

    #[test]
    fn refdes_skipped_by_scan_nodes() {
        // R1 without colon notation is not a port — scan_nodes skips it
        let input = "R1\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn word_not_a_refdes() {
        // "READ" is not a valid refdes (no digits after letters)
        let input = "READ\n";
        let anchors = find_refdes_anchors(input);
        assert!(anchors.is_empty());
    }

    #[test]
    fn empty_input() {
        let nodes = scan_nodes("");
        assert!(nodes.is_empty());
        let anchors = find_refdes_anchors("");
        assert!(anchors.is_empty());
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
        assert_eq!(nodes[0].text_width, 9);
    }

    #[test]
    fn mixed_nodes_on_one_line() {
        let input = "[VCC]  *  +  [GND]\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].node_type, NodeType::Label("VCC".to_string()));
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });
        assert_eq!(nodes[1].node_type, NodeType::Junction);
        assert_eq!(nodes[1].pos.col, 7);
        assert_eq!(nodes[2].node_type, NodeType::Corner);
        assert_eq!(nodes[2].pos.col, 10);
        assert_eq!(nodes[3].node_type, NodeType::Label("GND".to_string()));
        assert_eq!(nodes[3].pos.col, 13);
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

    fn full_tmpl_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<MatchedComponent>, Vec<String>) {
        let mut nodes = scan_nodes(input);
        let anchors = find_refdes_anchors(input);
        let lib = opa330_library();
        let refdes_map: HashMap<String, String> = [
            ("U1".to_string(), "OPA330xxD".to_string()),
        ].into_iter().collect();
        let (pin_nodes, mut matched, errors, _reass) = match_templates(&anchors, input, &refdes_map, &lib, &std::collections::HashSet::new());
        nodes.extend(pin_nodes);
        compress_coordinates(&mut nodes);
        resolve_matched_grid_positions(&mut matched, &nodes);
        (nodes, matched, errors)
    }

    #[test]
    fn opamp_rigid_match_succeeds() {
        let input = super::dag_layout_tests::new_opa330_sch();
        let (_nodes, matched, errors) = full_tmpl_pipeline(&input);

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(matched.len(), 1, "expected 1 matched component");
        let m = &matched[0];
        assert_eq!(m.refdes, "U1");
        assert!(m.symbol_name.contains("OPA330xxD"));

        // Verify all 5 visible pins are matched
        let pin_nums: Vec<usize> = m.pins.iter().map(|p| p.pin_num).collect();
        assert!(pin_nums.contains(&2));
        assert!(pin_nums.contains(&3));
        assert!(pin_nums.contains(&4));
        assert!(pin_nums.contains(&6));
        assert!(pin_nums.contains(&7));
        assert!(!m.draw_primitives.is_empty());
    }

    #[test]
    fn undeclared_refdes_is_rejected() {
        // Bare R1 in the grid without header declaration
        let input = "<R1>\n";
        let anchors = find_refdes_anchors(input);
        assert_eq!(anchors.len(), 1);
        let lib = HashMap::new();
        let refdes_map: HashMap<String, String> = HashMap::new();
        let (_pin_nodes, _matched, errors, _reass) = match_templates(&anchors, input, &refdes_map, &lib, &std::collections::HashSet::new());

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
        let (header, body, offset) = split_header_body(input);
        assert_eq!(header, "U1: OPA330xxD\nR1: R");
        assert_eq!(body, "[VCC] R1:1< R1:2>");
        assert_eq!(offset, 3, "body should start at line 3 (0-based)");
    }

    #[test]
    fn split_header_body_with_long_separator() {
        let input = "U1: OPA330xxD\n==================\nbody text";
        let (header, body, offset) = split_header_body(input);
        assert_eq!(header, "U1: OPA330xxD");
        assert_eq!(body, "body text");
        assert_eq!(offset, 2, "body should start at line 2 (0-based)");
    }

    #[test]
    fn split_header_body_without_separator() {
        let input = "just body text\nR1:1< R1:2>";
        let (header, body, offset) = split_header_body(input);
        assert_eq!(header, "");
        assert_eq!(body, "just body text\nR1:1< R1:2>");
        assert_eq!(offset, 0, "no separator means body starts at line 0");
    }

    #[test]
    fn split_header_body_empty_input() {
        let (header, body, offset) = split_header_body("");
        assert_eq!(header, "");
        assert_eq!(body, "");
        assert_eq!(offset, 0);
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

    pub(super) fn new_opa330_sch() -> String {
        // OPA330 0° compact (U1 at row 2, col 2):
        // P3 '<' at (-1,-2), P7 '^' at (-2,-1), P6 '>' at (0,1,AtRight),
        // P2 '<' at (1,-2), P4 'v' at (2,-1)
        let mut lines = Vec::new();
        lines.push(" ^".to_string());       // row 0: ^ at col 1
        lines.push("<".to_string());        // row 1: < at col 0
        lines.push("  U1>".to_string());    // row 2: U1 at col 2, > at col 4
        lines.push("<".to_string());        // row 3: < at col 0
        lines.push(" v".to_string());       // row 4: v at col 1
        lines.join("\n")
    }

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<MatchedComponent>, Vec<f64>, Vec<f64>) {
        let mut nodes = scan_nodes(input);
        let anchors = find_refdes_anchors(input);
        let lib = opa330_library();
        let refdes_map = opa330_refdes_map();
        let (pin_nodes, mut matched, _, _reass) = match_templates(&anchors, input, &refdes_map, &lib, &std::collections::HashSet::new());
        nodes.extend(pin_nodes);
        compress_coordinates(&mut nodes);
        resolve_matched_grid_positions(&mut matched, &nodes);
        compute_spans(&mut nodes);
        let (col_x, row_y) = compute_layout(&nodes, &matched);
        (nodes, matched, col_x, row_y)
    }

    #[test]
    fn opamp_pin_row_spacing_enforced() {
        let input = new_opa330_sch();
        let (_nodes, matched, _col_x, row_y) = full_pipeline(&input);
        println!("Full input:\n{}\n---", input);
        let anchors = find_refdes_anchors(&input);
        println!("Anchors: {:?}", anchors);
        println!("Matched count: {}", matched.len());
        let m = &matched[0];

        let p3 = m.pins.iter().find(|p| p.pin_num == 3).unwrap();
        let p2 = m.pins.iter().find(|p| p.pin_num == 2).unwrap();

        let dy = (row_y[p2.grid_row] as f64 - row_y[p3.grid_row] as f64).abs();
        // OPA330xxD: pin 2 is 5.08 mm from anchor (2 × 2.54)
        assert!(dy >= 5.07,
            "pin3→pin2 row spacing: {:.3} mm, template requires >= 5.08 mm", dy);
    }

    #[test]
    fn opamp_pin_col_spacing_enforced() {
        let input = new_opa330_sch();
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
        let input = new_opa330_sch();
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
        // OPA330 0° compact + [WIDE] label.
        let mut lines: Vec<String> = Vec::new();
        lines.push(" ^  [WIDE]".to_string());
        lines.push("<".to_string());
        lines.push("  U1>".to_string());
        lines.push("<".to_string());
        lines.push(" v".to_string());
        let input = lines.join("\n");

        let (_nodes, matched, col_x, row_y) = full_pipeline(&input);
        let m = &matched[0];

        let p3 = m.pins.iter().find(|p| p.pin_num == 3).unwrap();
        let p2 = m.pins.iter().find(|p| p.pin_num == 2).unwrap();
        let dy = (row_y[p2.grid_row] as f64 - row_y[p3.grid_row] as f64).abs();
        assert!(dy >= 5.07,
            "pin3→pin2 row spacing: {:.3} mm, template requires >= 5.08 mm", dy);

        let p6 = m.pins.iter().find(|p| p.pin_num == 6).unwrap();
        let dx = (col_x[p6.grid_col] as f64 - col_x[p3.grid_col] as f64).abs();
        assert!(dx >= 15.23,
            "pin3→pin6 col spacing: {:.3} mm, template requires >= 15.24 mm", dx);

        let p7 = m.pins.iter().find(|p| p.pin_num == 7).unwrap();
        let dx01 = (col_x[p7.grid_col] as f64 - col_x[p3.grid_col] as f64).abs();
        assert!(dx01 >= 5.07,
            "pin7→pin3 col spacing: {:.3} mm, template requires >= 5.08 mm", dx01);
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


          *\
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
        let junc = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction not found");

        assert_eq!(vcc.grid_row, 0, "VCC grid_row");
        assert_eq!(vcc.grid_col, 0, "VCC grid_col");

        assert_eq!(corner.grid_row, 0, "Corner grid_row");
        assert_eq!(corner.grid_col, 2, "Corner grid_col");

        assert_eq!(junc.grid_row, 1, "Junction grid_row");
        assert_eq!(junc.grid_col, 1, "Junction grid_col");
    }

    #[test]
    fn compress_single_row_all_same_row() {
        let input = "[A]  *  +  [B]\n";
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

    // All ports now use symmetric HALF_SPAN; DAG handles spacing.
    // The span tests use labels/junctions/corners/placeholders since
    // Port nodes are created by template matching, not scan_nodes.
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

        let half_w = (5.0 * crate::CHAR_WIDTH) / 2.0;
        assert!((lbl.span.left - half_w).abs() < 0.01);
        assert!((lbl.span.right - half_w).abs() < 0.01);
        assert_eq!(lbl.span.up, HALF_SPAN);
        assert_eq!(lbl.span.down, HALF_SPAN);
    }

    #[test]
    fn placeholder_span_is_symmetric() {
        let nodes = full_pipeline(".\n");
        let p = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Placeholder))
            .expect("Placeholder not found");

        assert_eq!(p.span.left, HALF_SPAN);
        assert_eq!(p.span.right, HALF_SPAN);
        assert_eq!(p.span.up, HALF_SPAN);
        assert_eq!(p.span.down, HALF_SPAN);
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
[VERY_LONG_SIGNAL_NAME_A]   *   [VCC]\
";
        let (_nodes, col_x, _row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 3);
        assert_eq!(col_x[0], MARGIN);

        let long_half = (23.0 + 2.0) * crate::CHAR_WIDTH / 2.0;
        let expected_long = long_half + HALF_SPAN + MIN_GAP;
        let gap_long = col_x[1] - col_x[0];
        assert!((gap_long - expected_long).abs() < 0.01,
            "long label gap expected {:.3}, got {:.3}", expected_long, gap_long);

        let short_half = ((3.0 + 2.0) * crate::CHAR_WIDTH) / 2.0;
        let expected_short = HALF_SPAN + short_half + MIN_GAP;
        let gap_short = col_x[2] - col_x[1];
        assert!((gap_short - expected_short).abs() < 0.01,
            "short label gap expected {:.3}, got {:.3}", expected_short, gap_short);

        assert!(gap_long > gap_short * 2.0,
            "long label gap ({:.3}) should be > 2x short label gap ({:.3})",
            gap_long, gap_short);
    }

    #[test]
    fn no_physical_overlap_between_bounding_boxes() {
        let input = "\
[VERY_LONG_SIGNAL_NAME_A]   *   [VCC]\
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
*
+\
";
        let (_nodes, _col_x, row_y) = full_pipeline(input);

        assert_eq!(row_y.len(), 3);
        assert_eq!(row_y[0], MARGIN);

        let expected0 = HALF_SPAN + HALF_SPAN + MIN_GAP;
        let gap0 = row_y[1] - row_y[0];
        assert!((gap0 - expected0).abs() < 0.01,
            "R0→R1 gap expected {:.2}, got {:.2}", expected0, gap0);

        let expected1 = HALF_SPAN + HALF_SPAN + MIN_GAP;
        let gap1 = row_y[2] - row_y[1];
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
         *
         [GND]\
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
              +
              [GND]\
";
        let (nodes, _col_x, _row_y, wires) = full_pipeline(input);

        assert_eq!(wires.len(), 3, "expected 3 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires (VCC→*, *→OUT)");
        assert_eq!(v_wires.len(), 1, "expected 1 vertical wire (*→+)");

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
