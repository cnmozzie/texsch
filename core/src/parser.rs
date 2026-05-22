// ============================================================
// Step 1: Node Identification & Absolute Coordinate Extraction
// Port-based Grid Architecture
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
    /// A component port, e.g. "R1:1", "C10:2"
    Port { refdes: String, pin: usize },
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

/// Base half-span for junctions, corners, and wire segments.
pub const HALF_SPAN: f64 = 20.0;

/// Minimum safe clearance between two adjacent span bounding boxes.
pub const MIN_GAP: f64 = 0.0;

/// SVG margin (px) from origin to first grid line.
pub const MARGIN: f64 = 60.0;

// ============================================================
// Grid conversion
// ============================================================

fn to_grid(input: &str) -> Vec<Vec<char>> {
    input.lines().map(|line| line.chars().collect()).collect()
}

// ============================================================
// Static scanner
// ============================================================

/// Walk the grid row by row, column by column, identifying
/// Labels `[...]`, Junctions `*`, Corners `+`, and Ports `R1:1`.
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
                'R' | 'L' | 'C' => {
                    if let Some((refdes, pin, width)) = try_parse_port(line, col) {
                        nodes.push(SchematicNode {
                            node_type: NodeType::Port { refdes, pin },
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

/// Try to parse a Port pattern at `start`:
///   Letter (R|L|C) + Digits + ':' + Digits
///
/// Returns (refdes, pin_number, total_width) on success.
fn try_parse_port(line: &[char], start: usize) -> Option<(String, usize, usize)> {
    let mut pos = start + 1;

    // Must have at least one digit after the type letter
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
    let width = pos - start;

    Some((refdes, pin, width))
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

/// Orientation of a placed two-pin component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// A validated component ready for instantiation in SVG / KiCad.
#[derive(Debug, Clone)]
pub struct PlacedComponent {
    pub refdes: String,
    pub comp_type: crate::CompType,
    pub orientation: Orientation,
    /// Grid-space centre column (may be fractional, e.g. 1.5).
    pub center_col: f64,
    /// Grid-space centre row (may be fractional).
    pub center_row: f64,
}

/// Pair ports by refdes, validate adjacency, and return placed components.
///
/// Returns `(placed, errors)` — components that failed validation are
/// reported in `errors` and omitted from `placed`.
pub fn pair_components(nodes: &[SchematicNode]) -> (Vec<PlacedComponent>, Vec<String>) {
    let mut groups: std::collections::BTreeMap<String, Vec<&SchematicNode>> =
        std::collections::BTreeMap::new();

    for node in nodes {
        if let NodeType::Port { refdes, .. } = &node.node_type {
            groups.entry(refdes.clone()).or_default().push(node);
        }
    }

    let mut placed = Vec::new();
    let mut errors = Vec::new();

    for (refdes, ports) in &groups {
        if ports.len() != 2 {
            errors.push(format!(
                "{}: expected 2 ports, found {}",
                refdes,
                ports.len()
            ));
            continue;
        }

        // Identify pin 1 and pin 2
        let (p1, p2) = {
            let mut p1 = None;
            let mut p2 = None;
            for port in ports {
                if let NodeType::Port { pin, .. } = &port.node_type {
                    match *pin {
                        1 => p1 = Some(*port),
                        2 => p2 = Some(*port),
                        _ => {}
                    }
                }
            }
            match (p1, p2) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    errors.push(format!(
                        "{}: must have exactly pin 1 and pin 2",
                        refdes
                    ));
                    continue;
                }
            }
        };

        let dr = (p1.grid_row as isize - p2.grid_row as isize).unsigned_abs();
        let dc = (p1.grid_col as isize - p2.grid_col as isize).unsigned_abs();

        let orientation = match (dr, dc) {
            (0, 1) => Orientation::Horizontal,
            (1, 0) => Orientation::Vertical,
            _ => {
                errors.push(format!(
                    "{}: pins are not adjacent (pin1 at R{}C{}, pin2 at R{}C{})",
                    refdes, p1.grid_row, p1.grid_col, p2.grid_row, p2.grid_col
                ));
                continue;
            }
        };

        let center_col = (p1.grid_col + p2.grid_col) as f64 / 2.0;
        let center_row = (p1.grid_row + p2.grid_row) as f64 / 2.0;

        let comp_type = match crate::CompType::from_char(refdes.chars().next().unwrap()) {
            Some(ct) => ct,
            None => {
                errors.push(format!("{}: unknown component type", refdes));
                continue;
            }
        };

        placed.push(PlacedComponent {
            refdes: refdes.clone(),
            comp_type,
            orientation,
            center_col,
            center_row,
        });
    }

    (placed, errors)
}

// ============================================================
// Step 3.5: Four-Direction Span Computation
// ============================================================

/// Compute the [`NodeSpan`] for every node based on its type and,
/// for ports, the orientation of the component it belongs to.
///
/// Must be called after [`pair_components`] so that component
/// orientations are known.
pub fn compute_spans(nodes: &mut [SchematicNode], placed: &[PlacedComponent]) {
    let orient_map: std::collections::HashMap<&str, Orientation> = placed
        .iter()
        .map(|c| (c.refdes.as_str(), c.orientation))
        .collect();

    let comp_hh: f64 = 15.0; // cross-direction half-extent

    for node in nodes.iter_mut() {
        node.span = match &node.node_type {
            // Rule A: junctions and corners are symmetric
            NodeType::Junction | NodeType::Corner => NodeSpan {
                left: HALF_SPAN,
                right: HALF_SPAN,
                up: HALF_SPAN,
                down: HALF_SPAN,
            },

            // Rule B: labels are symmetric text-based;
            // vertical extent is at least HALF_SPAN for wire clearance.
            NodeType::Label(name) => {
                let text_w = (name.len() + 2) as f64 * crate::CHAR_WIDTH;
                let text_h_half = (crate::LABEL_TEXT_H / 2.0).max(HALF_SPAN);
                NodeSpan {
                    left: text_w / 2.0,
                    right: text_w / 2.0,
                    up: text_h_half,
                    down: text_h_half,
                }
            }

            // Rule C: ports depend on component orientation and pin number
            NodeType::Port { refdes, pin } => {
                let comp_hw = crate::CompType::from_char(refdes.chars().next().unwrap())
                    .map(|ct| ct.symbol_half_width())
                    .unwrap_or(30.0);

                match (orient_map.get(refdes.as_str()), pin) {
                    // Shape 1: horizontal, left pin → body on the right
                    (Some(Orientation::Horizontal), 1) => NodeSpan {
                        left: HALF_SPAN,
                        right: comp_hw,
                        up: comp_hh,
                        down: comp_hh,
                    },
                    // Shape 2: horizontal, right pin → body on the left
                    (Some(Orientation::Horizontal), 2) => NodeSpan {
                        left: comp_hw,
                        right: HALF_SPAN,
                        up: comp_hh,
                        down: comp_hh,
                    },
                    // Shape 3: vertical, top pin → body below
                    (Some(Orientation::Vertical), 1) => NodeSpan {
                        left: comp_hh,
                        right: comp_hh,
                        up: HALF_SPAN,
                        down: comp_hw,
                    },
                    // Shape 4: vertical, bottom pin → body above
                    (Some(Orientation::Vertical), 2) => NodeSpan {
                        left: comp_hh,
                        right: comp_hh,
                        up: comp_hw,
                        down: HALF_SPAN,
                    },
                    // Orphan port (no paired component) → treat as junction
                    _ => NodeSpan {
                        left: HALF_SPAN,
                        right: HALF_SPAN,
                        up: HALF_SPAN,
                        down: HALF_SPAN,
                    },
                }
            }
        };
    }
}

// ============================================================
// Step 4: Dynamic Grid Layout
// ============================================================

/// Compute dynamic physical coordinates for grid columns and rows.
///
/// Returns `(col_x, row_y)` where:
/// * `col_x[c]` – x-coordinate (SVG px) of grid column *c*
/// * `row_y[r]` – y-coordinate (SVG px) of grid row *r*
///
/// Spacing between adjacent columns / rows is determined by the maximum
/// span extent in that direction plus [`MIN_GAP`].
pub fn compute_layout(nodes: &[SchematicNode]) -> (Vec<f64>, Vec<f64>) {
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

    // --- column x-coordinates ---
    let mut col_x = vec![MARGIN; max_col + 1];
    for c in 0..max_col {
        let mut max_r = HALF_SPAN;
        let mut max_l = HALF_SPAN;
        for r in 0..=max_row {
            max_r = max_r.max(span_or_default(r, c, |s| s.right));
            max_l = max_l.max(span_or_default(r, c + 1, |s| s.left));
        }
        col_x[c + 1] = col_x[c] + max_r + max_l + MIN_GAP;
    }

    // --- row y-coordinates ---
    let mut row_y = vec![MARGIN; max_row + 1];
    for r in 0..max_row {
        let mut max_d = HALF_SPAN;
        let mut max_u = HALF_SPAN;
        for c in 0..=max_col {
            max_d = max_d.max(span_or_default(r, c, |s| s.down));
            max_u = max_u.max(span_or_default(r + 1, c, |s| s.up));
        }
        row_y[r + 1] = row_y[r] + max_d + max_u + MIN_GAP;
    }

    (col_x, row_y)
}

/// Physical centre of a placed component given the dynamic layout arrays.
pub fn component_physical_center(
    comp: &PlacedComponent,
    col_x: &[f64],
    row_y: &[f64],
) -> (f64, f64) {
    match comp.orientation {
        Orientation::Horizontal => {
            let ci = comp.center_col.floor() as usize;
            let cx = (col_x[ci] + col_x[ci + 1]) / 2.0;
            let cy = row_y[comp.center_row as usize];
            (cx, cy)
        }
        Orientation::Vertical => {
            let ri = comp.center_row.floor() as usize;
            let cx = col_x[comp.center_col as usize];
            let cy = (row_y[ri] + row_y[ri + 1]) / 2.0;
            (cx, cy)
        }
    }
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

/// Half the SVG symbol span (matches `SYMBOL_SPAN` in svg.rs: 56.0 / 2).
pub const SYMBOL_HALF: f64 = 28.0;

/// Determine the physical connection point for any node.
///
/// * Ports → symbol pin position (component centre ± [`SYMBOL_HALF`]).
/// * Labels → edge of the text bounding box in the wire direction.
/// * Junctions / Corners → grid centre.
fn endpoint_position(
    node: &SchematicNode,
    placed: &[PlacedComponent],
    col_x: &[f64],
    row_y: &[f64],
    is_horizontal: bool,
    is_first: bool, // true = left (H) / top (V)
) -> (f64, f64) {
    match &node.node_type {
        NodeType::Port { refdes, pin } => {
            // Look up the placed component to get centre and orientation.
            if let Some(comp) = placed.iter().find(|c| c.refdes == *refdes) {
                let (cx, cy) = component_physical_center(comp, col_x, row_y);
                match (comp.orientation, pin) {
                    (Orientation::Horizontal, 1) => (cx - SYMBOL_HALF, cy),
                    (Orientation::Horizontal, 2) => (cx + SYMBOL_HALF, cy),
                    (Orientation::Vertical, 1) => (cx, cy - SYMBOL_HALF),
                    (Orientation::Vertical, 2) => (cx, cy + SYMBOL_HALF),
                    _ => (col_x[node.grid_col], row_y[node.grid_row]),
                }
            } else {
                // Orphan port — fall back to grid centre.
                (col_x[node.grid_col], row_y[node.grid_row])
            }
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
    placed: &[PlacedComponent],
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
            let ascii_row = a.pos.row;
            let c1 = a.pos.col.min(b.pos.col);
            let c2 = a.pos.col.max(b.pos.col);

            let has_dash = (c1..c2).any(|col| {
                grid.get(ascii_row)
                    .and_then(|line| line.get(col))
                    .is_some_and(|&ch| ch == '-')
            });

            if has_dash {
                let (x1, y1) = endpoint_position(a, placed, col_x, row_y, true, true);
                let (x2, y2) = endpoint_position(b, placed, col_x, row_y, true, false);
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
            let ascii_col = a.pos.col;
            let r1 = a.pos.row.min(b.pos.row);
            let r2 = a.pos.row.max(b.pos.row);

            let has_pipe = (r1..r2).any(|row| {
                grid.get(row)
                    .and_then(|line| line.get(ascii_col))
                    .is_some_and(|&ch| ch == '|')
            });

            if has_pipe {
                let (x1, y1) = endpoint_position(a, placed, col_x, row_y, false, true);
                let (x2, y2) = endpoint_position(b, placed, col_x, row_y, false, false);
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
[VCC]  +  R1:1
          *
          C1:1\
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

        // (0, 10) Port "R1:1"
        let r1 = &nodes[2];
        assert_eq!(r1.pos, AbsPos { row: 0, col: 10 });
        assert_eq!(r1.text_width, 4);
        assert_eq!(
            r1.node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1
            }
        );

        // (1, 10) Junction "*"
        let junc = &nodes[3];
        assert_eq!(junc.pos, AbsPos { row: 1, col: 10 });
        assert_eq!(junc.text_width, 1);
        assert_eq!(junc.node_type, NodeType::Junction);

        // (2, 10) Port "C1:1"
        let c1 = &nodes[4];
        assert_eq!(c1.pos, AbsPos { row: 2, col: 10 });
        assert_eq!(c1.text_width, 4);
        assert_eq!(
            c1.node_type,
            NodeType::Port {
                refdes: "C1".to_string(),
                pin: 1
            }
        );

        // Total node count
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn scan_port_with_multi_digit_refdes() {
        let input = "R10:2  C100:1\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 2);

        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "R10".to_string(),
                pin: 2
            }
        );
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });
        assert_eq!(nodes[0].text_width, 5); // R10:2

        assert_eq!(
            nodes[1].node_type,
            NodeType::Port {
                refdes: "C100".to_string(),
                pin: 1
            }
        );
        assert_eq!(nodes[1].pos, AbsPos { row: 0, col: 7 });
        assert_eq!(nodes[1].text_width, 6); // C100:1
    }

    #[test]
    fn scan_inductor_port() {
        let input = "L3:1\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "L3".to_string(),
                pin: 1
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
        // "READ" is a word, not a port
        let input = "READ\n";
        let nodes = scan_nodes(input);
        assert!(nodes.is_empty());
    }

    #[test]
    fn port_with_trailing_text() {
        // "R1:1abc" — the port is R1:1, trailing "abc" is skipped
        let input = "R1:1abc\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1
            }
        );
        assert_eq!(nodes[0].text_width, 4);
    }

    #[test]
    fn mixed_nodes_on_one_line() {
        let input = "[VCC]  R1:1  *  +  [GND]\n";
        let nodes = scan_nodes(input);

        assert_eq!(nodes.len(), 5);

        assert_eq!(nodes[0].node_type, NodeType::Label("VCC".to_string()));
        assert_eq!(nodes[0].pos, AbsPos { row: 0, col: 0 });

        assert_eq!(
            nodes[1].node_type,
            NodeType::Port {
                refdes: "R1".to_string(),
                pin: 1
            }
        );
        assert_eq!(nodes[1].pos.col, 7);

        assert_eq!(nodes[2].node_type, NodeType::Junction);

        assert_eq!(nodes[3].node_type, NodeType::Corner);

        assert_eq!(nodes[4].node_type, NodeType::Label("GND".to_string()));
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
}

// ============================================================
// Step 2 Coordinate Compression Tests
// ============================================================
#[cfg(test)]
mod step2_compression_tests {
    use super::*;

    #[test]
    fn compress_with_empty_lines_and_sparse_columns() {
        // Input with an empty row and large gaps between columns.
        // [VCC] at abs (0,0), + at abs (0,15), R1:1 at abs (2,10)
        let input = "\
[VCC]          +

          R1:1\
";
        let mut nodes = scan_nodes(input);
        assert_eq!(nodes.len(), 3, "expected 3 nodes");

        compress_coordinates(&mut nodes);

        // Build lookup by node type for easy assertion
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

        // [VCC] → abs (0,0) → grid (0,0)
        assert_eq!(vcc.grid_row, 0, "VCC grid_row");
        assert_eq!(vcc.grid_col, 0, "VCC grid_col");

        // + → abs (0,15) → grid (0,2)  (cols: 0,10,15 → index 2)
        assert_eq!(corner.grid_row, 0, "Corner grid_row");
        assert_eq!(corner.grid_col, 2, "Corner grid_col");

        // R1:1 → abs (2,10) → grid (1,1)  (rows: 0,2 → index 1; cols: 0,10,15 → index 1)
        assert_eq!(r1.grid_row, 1, "R1:1 grid_row");
        assert_eq!(r1.grid_col, 1, "R1:1 grid_col");
    }

    #[test]
    fn compress_single_row_all_same_row() {
        let input = "[A]  *  +  R1:1\n";
        let mut nodes = scan_nodes(input);
        assert_eq!(nodes.len(), 4);

        compress_coordinates(&mut nodes);

        // All on same row → grid_row = 0
        for node in &nodes {
            assert_eq!(node.grid_row, 0);
        }
        // Columns distinct and sorted → grid_col increments
        let cols: Vec<usize> = nodes.iter().map(|n| n.grid_col).collect();
        let mut expected = cols.clone();
        expected.sort();
        assert_eq!(cols, expected, "grid_col should be monotonic left-to-right");
    }

    #[test]
    fn compress_empty_nodes() {
        let mut nodes: Vec<SchematicNode> = vec![];
        compress_coordinates(&mut nodes);
        // Just shouldn't panic
    }
}

// ============================================================
// Step 3 Pairing Tests
// ============================================================
#[cfg(test)]
mod step3_pairing_tests {
    use super::*;

    fn scan_and_compress(input: &str) -> Vec<SchematicNode> {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        nodes
    }

    #[test]
    fn horizontal_resistor_and_vertical_capacitor() {
        // R1:1 and R1:2 on same row, adjacent cols → Horizontal
        // C1:1 and C1:2 on same col, adjacent rows → Vertical
        let input = "\
[VCC]   R1:1   R1:2
        C1:1
        C1:2\
";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(placed.len(), 2);

        // R1: horizontal
        let r1 = placed.iter().find(|c| c.refdes == "R1").expect("R1 not found");
        assert_eq!(r1.orientation, Orientation::Horizontal);
        assert_eq!(r1.comp_type, crate::CompType::Resistor);
        // R1:1 at (R0,C1), R1:2 at (R0,C2) → center at (0.0, 1.5)
        assert_eq!(r1.center_row, 0.0);
        assert_eq!(r1.center_col, 1.5);

        // C1: vertical
        let c1 = placed.iter().find(|c| c.refdes == "C1").expect("C1 not found");
        assert_eq!(c1.orientation, Orientation::Vertical);
        assert_eq!(c1.comp_type, crate::CompType::Capacitor);
        // C1:1 at (R1,C1), C1:2 at (R2,C1) → center at (1.5, 1.0)
        assert_eq!(c1.center_row, 1.5);
        assert_eq!(c1.center_col, 1.0);
    }

    #[test]
    fn non_adjacent_pins_are_rejected() {
        // R2:1 at (0,0), R2:2 at (0,3) — not adjacent
        let input = "R2:1    +    R2:2\n";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(placed.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not adjacent"), "got: {}", errors[0]);
        assert!(errors[0].contains("R2"), "got: {}", errors[0]);
    }

    #[test]
    fn diagonal_pins_are_rejected() {
        // R3:1 at (0,0), R3:2 at (1,2) — diagonal with gap, not adjacent
        let input = "R3:1\n+  R3:2\n";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(placed.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not adjacent"));
    }

    #[test]
    fn single_port_is_rejected() {
        let input = "R4:1\n";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(placed.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("expected 2 ports"));
    }

    #[test]
    fn three_ports_same_refdes_rejected() {
        let input = "R5:1  R5:2  R5:1\n";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(placed.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("expected 2 ports"));
    }

    #[test]
    fn inductor_horizontal() {
        let input = "L1:1  L1:2\n";
        let nodes = scan_and_compress(input);
        let (placed, errors) = pair_components(&nodes);

        assert!(errors.is_empty());
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].refdes, "L1");
        assert_eq!(placed[0].comp_type, crate::CompType::Inductor);
        assert_eq!(placed[0].orientation, Orientation::Horizontal);
    }

    #[test]
    fn empty_nodes_yields_no_components() {
        let nodes: Vec<SchematicNode> = vec![];
        let (placed, errors) = pair_components(&nodes);
        assert!(placed.is_empty());
        assert!(errors.is_empty());
    }
}

// ============================================================
// Step 3.5 Span Computation Tests
// ============================================================
#[cfg(test)]
mod step35_span_tests {
    use super::*;

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<PlacedComponent>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        let (placed, _errors) = pair_components(&nodes);
        compute_spans(&mut nodes, &placed);
        (nodes, placed)
    }

    #[test]
    fn horizontal_resistor_pin1_span() {
        let (nodes, _placed) = full_pipeline("R1:1  R1:2\n");
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "R1" && *pin == 1))
            .expect("R1:1 not found");

        // Shape 1: horizontal left pin → body on the right
        assert_eq!(p1.span.left, HALF_SPAN);           // 20.0
        assert_eq!(p1.span.right, 30.0);               // resistor symbol_half_width
        assert_eq!(p1.span.up, 15.0);                  // comp_hh
        assert_eq!(p1.span.down, 15.0);
    }

    #[test]
    fn horizontal_resistor_pin2_span() {
        let (nodes, _placed) = full_pipeline("R1:1  R1:2\n");
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "R1" && *pin == 2))
            .expect("R1:2 not found");

        // Shape 2: horizontal right pin → body on the left
        assert_eq!(p2.span.left, 30.0);                // resistor symbol_half_width
        assert_eq!(p2.span.right, HALF_SPAN);           // 20.0
        assert_eq!(p2.span.up, 15.0);
        assert_eq!(p2.span.down, 15.0);
    }

    #[test]
    fn vertical_capacitor_pin1_span() {
        let input = "\
C1:1
C1:2\
";
        let (nodes, _placed) = full_pipeline(input);
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "C1" && *pin == 1))
            .expect("C1:1 not found");

        // Shape 3: vertical top pin → body below
        assert_eq!(p1.span.left, 15.0);
        assert_eq!(p1.span.right, 15.0);
        assert_eq!(p1.span.up, HALF_SPAN);              // 20.0
        assert_eq!(p1.span.down, 28.0);                 // capacitor symbol_half_width
    }

    #[test]
    fn vertical_capacitor_pin2_span() {
        let input = "\
C1:1
C1:2\
";
        let (nodes, _placed) = full_pipeline(input);
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "C1" && *pin == 2))
            .expect("C1:2 not found");

        // Shape 4: vertical bottom pin → body above
        assert_eq!(p2.span.left, 15.0);
        assert_eq!(p2.span.right, 15.0);
        assert_eq!(p2.span.up, 28.0);                   // capacitor symbol_half_width
        assert_eq!(p2.span.down, HALF_SPAN);             // 20.0
    }

    #[test]
    fn junction_span_is_symmetric() {
        let (nodes, _placed) = full_pipeline("*\n");
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
        let (nodes, _placed) = full_pipeline("+\n");
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
        let (nodes, _placed) = full_pipeline("[VCC]\n");
        let lbl = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Label(name) if name == "VCC"))
            .expect("VCC label not found");

        // text_w = (3 + 2) * 8.0 = 40.0 → half = 20.0
        // text_h = 12.0 → half = 6.0, clamped to HALF_SPAN = 20.0
        assert_eq!(lbl.span.left, 20.0);
        assert_eq!(lbl.span.right, 20.0);
        assert_eq!(lbl.span.up, HALF_SPAN);
        assert_eq!(lbl.span.down, HALF_SPAN);
    }

    #[test]
    fn orphan_port_without_component_uses_default() {
        // R9:1 has no pair → not in placed → span falls back to HALF_SPAN
        let (nodes, _placed) = full_pipeline("R9:1\n");
        let p = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "R9" && *pin == 1))
            .expect("R9:1 not found");

        assert_eq!(p.span.left, HALF_SPAN);
        assert_eq!(p.span.right, HALF_SPAN);
        assert_eq!(p.span.up, HALF_SPAN);
        assert_eq!(p.span.down, HALF_SPAN);
    }

    #[test]
    fn horizontal_inductor_pin_spans() {
        let (nodes, _placed) = full_pipeline("L1:1  L1:2\n");
        let p1 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "L1" && *pin == 1))
            .expect("L1:1 not found");
        let p2 = nodes
            .iter()
            .find(|n| matches!(&n.node_type, NodeType::Port { refdes, pin } if refdes == "L1" && *pin == 2))
            .expect("L1:2 not found");

        // Inductor symbol_half_width is 30.0 (same as resistor)
        assert_eq!(p1.span.right, 30.0);
        assert_eq!(p2.span.left, 30.0);
    }
}

// ============================================================
// Step 4 Dynamic Grid Layout Tests
// ============================================================
#[cfg(test)]
mod step4_layout_tests {
    use super::*;

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<PlacedComponent>, Vec<f64>, Vec<f64>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        let (placed, _errors) = pair_components(&nodes);
        compute_spans(&mut nodes, &placed);
        let (col_x, row_y) = compute_layout(&nodes);
        (nodes, placed, col_x, row_y)
    }

    #[test]
    fn long_label_widens_column_gap() {
        // [VERY_LONG_SIGNAL_NAME_A] is 23 chars + 2 brackets = 25 char width
        // text_w = 25 * 8.0 = 200, half = 100 px each side
        // [VCC] is 3 chars + 2 brackets = 5 char width
        // text_w = 5 * 8.0 = 40, half = 20 px each side
        let input = "\
[VERY_LONG_SIGNAL_NAME_A]   R1:1  R1:2   [VCC]\
";
        let (_nodes, _placed, col_x, _row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 4); // cols 0,1,2,3
        assert_eq!(col_x[0], MARGIN);

        // Gap C0→C1: long label (right=100) + R1:1 (left=20) + MIN_GAP(0) = 120
        let gap_long = col_x[1] - col_x[0];
        assert!((gap_long - 120.0).abs() < 0.01,
            "long label gap expected 120.0, got {:.1}", gap_long);

        // Gap C2→C3: R1:2 (right=20) + [VCC] (left=20) + MIN_GAP(0) = 40
        let gap_short = col_x[3] - col_x[2];
        assert!((gap_short - 40.0).abs() < 0.01,
            "short label gap expected 40.0, got {:.1}", gap_short);

        // Long label gap must be substantially larger than short label gap
        assert!(gap_long > gap_short * 2.0,
            "long label gap ({:.1}) should be > 2x short label gap ({:.1})",
            gap_long, gap_short);
    }

    #[test]
    fn no_physical_overlap_between_bounding_boxes() {
        let input = "\
[VERY_LONG_SIGNAL_NAME_A]   R1:1  R1:2   [VCC]\
";
        let (nodes, _placed, col_x, row_y) = full_pipeline(input);

        // Build lookup: (r,c) → span
        let node_at: std::collections::HashMap<(usize, usize), &SchematicNode> = nodes
            .iter()
            .map(|n| ((n.grid_row, n.grid_col), n))
            .collect();

        let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
        let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

        // Check every adjacent column pair: the rightmost box edge in column c
        // must have at least MIN_GAP clearance from the leftmost box edge in column c+1.
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

        // Same check for rows
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
C1:1
C1:2\
";
        let (_nodes, _placed, _col_x, row_y) = full_pipeline(input);

        assert_eq!(row_y.len(), 3); // rows 0,1,2
        assert_eq!(row_y[0], MARGIN);

        // Row 0: [VCC] (down = 20)
        // Row 1: C1:1 (up = 20 (HALF_SPAN)) — vertical pin 1 has up=HALF_SPAN=20
        // R0→R1 gap: 20 + 20 + 0 = 40
        let gap0 = row_y[1] - row_y[0];
        assert!((gap0 - 40.0).abs() < 0.01,
            "R0→R1 gap expected 40, got {:.1}", gap0);

        // Row 1: C1:1 (down = 28) — vertical pin 1 has down=comp_hw=28
        // Row 2: C1:2 (up = 28) — vertical pin 2 has up=comp_hw=28
        // R1→R2 gap: 28 + 28 + 0 = 56
        let gap1 = row_y[2] - row_y[1];
        assert!((gap1 - 56.0).abs() < 0.01,
            "R1→R2 gap expected 56, got {:.1}", gap1);
    }

    #[test]
    fn empty_input_layout() {
        let input = "";
        let (_nodes, _placed, col_x, row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 1);
        assert_eq!(col_x[0], MARGIN);
        assert_eq!(row_y.len(), 1);
        assert_eq!(row_y[0], MARGIN);
    }

    #[test]
    fn single_node_layout() {
        let input = "*\n";
        let (_nodes, _placed, col_x, row_y) = full_pipeline(input);

        assert_eq!(col_x.len(), 1);
        assert_eq!(row_y.len(), 1);
        assert_eq!(col_x[0], MARGIN);
        assert_eq!(row_y[0], MARGIN);
    }

    #[test]
    fn component_physical_center_horizontal() {
        let input = "R1:1  R1:2\n";
        let (_nodes, placed, col_x, row_y) = full_pipeline(input);

        let comp = placed.iter().find(|c| c.refdes == "R1").unwrap();
        let (cx, cy) = component_physical_center(comp, &col_x, &row_y);

        // Horizontal: ports at (r=0, c=0) and (r=0, c=1)
        // center should be at midpoint of col_x[0] and col_x[1], same row
        assert!((cx - (col_x[0] + col_x[1]) / 2.0).abs() < 0.01);
        assert!((cy - row_y[0]).abs() < 0.01);
    }

    #[test]
    fn component_physical_center_vertical() {
        let input = "\
C1:1
C1:2\
";
        let (_nodes, placed, col_x, row_y) = full_pipeline(input);

        let comp = placed.iter().find(|c| c.refdes == "C1").unwrap();
        let (cx, cy) = component_physical_center(comp, &col_x, &row_y);

        // Vertical: ports at (r=0, c=0) and (r=1, c=0)
        // center should be at midpoint of row_y[0] and row_y[1], same col
        assert!((cx - col_x[0]).abs() < 0.01);
        assert!((cy - (row_y[0] + row_y[1]) / 2.0).abs() < 0.01);
    }
}

// ============================================================
// Step 5: Grid-Neighbour Wire Extraction Tests
// ============================================================
#[cfg(test)]
mod step5_wire_tests {
    use super::*;

    fn full_pipeline(input: &str) -> (Vec<SchematicNode>, Vec<PlacedComponent>, Vec<f64>, Vec<f64>, Vec<WireSegment>) {
        let mut nodes = scan_nodes(input);
        compress_coordinates(&mut nodes);
        let (placed, _errors) = pair_components(&nodes);
        compute_spans(&mut nodes, &placed);
        let (col_x, row_y) = compute_layout(&nodes);
        let wires = extract_wires(&nodes, &placed, &col_x, &row_y, input);
        (nodes, placed, col_x, row_y, wires)
    }

    // ---------------------------------------------------------------
    // Test 1: Corner — horizontal line to '+', vertical line from '+' to R1
    // ---------------------------------------------------------------
    #[test]
    fn corner_wire_routing() {
        // + at col 9, | at col 9, R1:1/R1:2 at col 9
        let input = "\
[VCC] ---+
         |
         R1:1
         R1:2\
";
        let (nodes, _placed, _col_x, _row_y, wires) = full_pipeline(input);

        // Should have 2 wires: 1 horizontal ([VCC] → +), 1 vertical (+ → R1:1)
        assert_eq!(wires.len(), 2, "expected 2 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 1, "expected 1 horizontal wire");
        assert_eq!(v_wires.len(), 1, "expected 1 vertical wire");

        // '+' is a Corner node — must NOT be a Junction
        let corner = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner node not found");
        assert!(!matches!(corner.node_type, NodeType::Junction),
            "'+' is a Corner, not a Junction — no dot should be drawn");
    }

    // ---------------------------------------------------------------
    // Test 2: Crossing without Connection — '+' is Corner, no dot
    // ---------------------------------------------------------------
    #[test]
    fn crossing_without_connection() {
        // + at col 9; [Y1], |, [Y2] all at col 9
        let input = "         [Y1]
         |
[X1] ----+---- [X2]
         |
         [Y2]";
        let (nodes, _placed, _col_x, _row_y, wires) = full_pipeline(input);

        // 4 wires: Y1→+, +→Y2, X1→+, +→X2
        assert_eq!(wires.len(), 4, "expected 4 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires");
        assert_eq!(v_wires.len(), 2, "expected 2 vertical wires");

        // The centre node is '+' (Corner), NOT '*' — absolutely no Junction dot
        let corner = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Corner))
            .expect("Corner node not found");
        assert!(!matches!(corner.node_type, NodeType::Junction));

        // Also verify there is NO Junction node at all in this input
        let has_junction = nodes.iter().any(|n| matches!(n.node_type, NodeType::Junction));
        assert!(!has_junction, "crossing test must not contain any Junction (*)");
    }

    // ---------------------------------------------------------------
    // Test 3: T-Junction — '*' has a Junction dot
    // ---------------------------------------------------------------
    #[test]
    fn t_junction_with_dot() {
        // * at col 14; |, R2:1, R2:2 all at col 14
        let input = "\
[VCC] ------- * ------- [OUT]
              |
              R2:1
              R2:2\
";
        let (nodes, _placed, _col_x, _row_y, wires) = full_pipeline(input);

        // 3 wires meeting at '*': VCC→*, *→OUT, *→R2:1
        assert_eq!(wires.len(), 3, "expected 3 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires (VCC→*, *→OUT)");
        assert_eq!(v_wires.len(), 1, "expected 1 vertical wire (*→R2:1)");

        // The centre node is '*' (Junction) — MUST have a dot
        let junction = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction node (*) not found");
        assert!(matches!(junction.node_type, NodeType::Junction),
            "'*' must be a Junction so a dot is drawn");
    }

    // ---------------------------------------------------------------
    // Test 4: Cross-Junction — '*' has a Junction dot, 4 arms
    // ---------------------------------------------------------------
    #[test]
    fn cross_junction_with_dot() {
        // * at col 11; [UP], |, [DOWN] all at col 11
        let input = "           [UP]
           |
[LEFT] --- * --- [RIGHT]
           |
           [DOWN]";
        let (nodes, _placed, _col_x, _row_y, wires) = full_pipeline(input);

        // 4 wires meeting at '*': LEFT→*, *→RIGHT, UP→*, *→DOWN
        assert_eq!(wires.len(), 4, "expected 4 wires, got {}: {:?}", wires.len(), wires);

        let h_wires: Vec<_> = wires.iter().filter(|w| w.is_horizontal()).collect();
        let v_wires: Vec<_> = wires.iter().filter(|w| !w.is_horizontal()).collect();
        assert_eq!(h_wires.len(), 2, "expected 2 horizontal wires");
        assert_eq!(v_wires.len(), 2, "expected 2 vertical wires");

        // Centre node '*' is Junction — MUST draw a dot
        let junction = nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Junction))
            .expect("Junction node (*) not found");
        assert!(matches!(junction.node_type, NodeType::Junction),
            "'*' must be a Junction so a dot is drawn");
    }
}
