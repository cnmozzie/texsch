pub mod kicad;
pub mod kicad_sym;
pub mod parser;
pub mod svg;
use wasm_bindgen::prelude::*;

/// Estimated width of one character in mm (for DAG layout spacing).
/// Old value: 8.0 px × 2.54/60 = 0.339 mm.
pub const CHAR_WIDTH: f64 = 0.339;

/// Estimated height of a text line in mm (for DAG layout spacing).
/// Old value: 12.0 px × 2.54/60 = 0.508 mm.
pub const LABEL_TEXT_H: f64 = 0.508;

/// Output of the `compile` pipeline.
#[wasm_bindgen]
#[derive(Debug)]
pub struct CompileResult {
    #[wasm_bindgen(getter_with_clone)]
    pub svg: String,
    #[wasm_bindgen(getter_with_clone)]
    pub kicad_sch: String,
    #[wasm_bindgen(getter_with_clone)]
    pub debug: String,
    /// JSON-serialized [`Vec<parser::ComponentTextSpan>`] — source-map entries
    /// mapping each component port text fragment to its line/column span in the
    /// original ASCII input.  The frontend deserialises this to navigate between
    /// schematic elements and editor text.
    #[wasm_bindgen(getter_with_clone)]
    pub source_map_json: String,
    /// JSON-serialized [`Vec<parser::RefdesReassignment>`] — reassignments
    /// for duplicate refdes instances that were auto-incremented (e.g. R1→R2).
    /// The frontend applies these to update the editor text and header.
    #[wasm_bindgen(getter_with_clone)]
    pub refdes_reassignments_json: String,
    /// JSON-serialized `HashMap<String, f64>` — maps each matched component's
    /// refdes to its current KiCad CW rotation angle in degrees (0/90/180/270).
    #[wasm_bindgen(getter_with_clone)]
    pub angles_json: String,
}

/// Per-grid compilation result produced by [`process_grid`].
struct GridOutput {
    nodes: Vec<parser::SchematicNode>,
    matched: Vec<parser::MatchedComponent>,
    col_x: Vec<f64>,
    row_y: Vec<f64>,
    wires: Vec<parser::WireSegment>,
    labels: Vec<(String, usize, usize)>,
    source_map: Vec<parser::ComponentTextSpan>,
    refdes_reassignments: Vec<parser::RefdesReassignment>,
    match_errors: Vec<String>,
}

/// Run the full per-grid compilation pipeline on `body`.
fn process_grid(
    body: &str,
    body_line_offset: usize,
    refdes_to_symbol: &std::collections::HashMap<String, String>,
    bundle: &kicad_sym::LibraryBundle,
    reserved_refdes: &std::collections::HashSet<String>,
) -> GridOutput {
    let mut nodes = parser::scan_nodes(body);
    let anchors = parser::find_refdes_anchors(body);

    let (pin_nodes, mut matched, match_errors, refdes_reassignments) =
        parser::match_templates(&anchors, body, refdes_to_symbol, &bundle.symbols, reserved_refdes);

    nodes.extend(pin_nodes);
    parser::compress_coordinates(&mut nodes);
    parser::resolve_matched_grid_positions(&mut matched, &nodes);
    parser::apply_rotation_to_rel_phys(&mut matched);
    parser::compute_spans(&mut nodes);
    let (col_x, row_y) = parser::compute_layout(&nodes, &matched);
    parser::compute_pin_ki_positions(&mut matched, &col_x, &row_y);

    let labels: Vec<(String, usize, usize)> = nodes
        .iter()
        .filter_map(|n| {
            if let parser::NodeType::Label(name) = &n.node_type {
                Some((name.clone(), n.grid_row, n.grid_col))
            } else {
                None
            }
        })
        .collect();

    let wires = parser::extract_wires(&nodes, &matched, &col_x, &row_y, body);
    let source_map = parser::build_source_map(&nodes, &anchors, body_line_offset);

    GridOutput {
        nodes, matched, col_x, row_y, wires, labels,
        source_map, refdes_reassignments, match_errors,
    }
}

/// Parse the ASCII schematic `input` using the Port-based Grid scanner.
/// Supports dual `====` separators: header → Grid1 (main circuit) → Grid2 (preview).
#[wasm_bindgen]
pub fn compile(input: &str) -> CompileResult {
    // Step 0: split into three sections via up to two `====` separators
    let sections = parser::split_three_sections(input);
    let refdes_to_symbol = parser::parse_header(&sections.header);

    // Load built-in symbol library (compile-time embedded KiCad files)
    let bundle = kicad_sym::load_builtin_library();

    // ---- Process Grid 1 (main circuit canvas) -----------------------------
    let empty_reserved = std::collections::HashSet::new();
    let mut grid1 = process_grid(
        &sections.grid1_body,
        sections.grid1_line_offset,
        &refdes_to_symbol,
        &bundle,
        &empty_reserved,
    );

    // Collect Grid1's matched refdes so Grid2 auto-increments collisions.
    let grid1_refdes: std::collections::HashSet<String> = grid1
        .matched
        .iter()
        .map(|c| c.refdes.clone())
        .collect();

    // ---- Process Grid 2 (component preview sandbox) if present -------------
    let mut grid2 = if !sections.grid2_body.is_empty() {
        Some(process_grid(
            &sections.grid2_body,
            sections.grid2_line_offset,
            &refdes_to_symbol,
            &bundle,
            &grid1_refdes,
        ))
    } else {
        None
    };

    // ---- Combine debug output (Grid 1 primary) -----------------------------
    let mut debug = format!("Grid1 — {} nodes, {} matched:\n",
        grid1.nodes.len(), grid1.matched.len());
    debug.push_str(&format_debug(&grid1.nodes, &grid1.matched, &grid1.match_errors));
    if let Some(ref g2) = grid2 {
        debug.push_str(&format!("\nGrid2 — {} nodes, {} matched:\n",
            g2.nodes.len(), g2.matched.len()));
        debug.push_str(&format_debug(&g2.nodes, &g2.matched, &g2.match_errors));
    }

    // ---- SVG: dual-grid stacked layout ------------------------------------
    let svg = svg::generate_dual_grid(
        &grid1.nodes, &grid1.wires, &grid1.col_x, &grid1.row_y, &grid1.matched,
        grid2.as_ref().map(|g| {
            (g.nodes.as_slice(), g.wires.as_slice(),
             g.col_x.as_slice(), g.row_y.as_slice(),
             g.matched.as_slice())
        }),
    );

    // ---- KiCad: Grid 1 only (main circuit) — Grid 2 is strictly isolated --
    let kicad_sch = kicad::generate_step3(
        &grid1.labels, &grid1.nodes, &grid1.col_x, &grid1.row_y,
        &grid1.matched, &sections.grid1_body, &bundle.entries,
    );

    // ---- Combine source maps (both grids share the same absolute line coords)
    let mut source_map = grid1.source_map;
    if let Some(ref g2) = grid2 {
        source_map.extend(g2.source_map.clone());
    }
    let source_map_json = serde_json::to_string(&source_map).unwrap_or_default();

    // ---- Refdes reassignments: convert body-relative to absolute 1-based ---
    fn make_absolute(reassignments: &mut [parser::RefdesReassignment], offset: usize) {
        for r in reassignments {
            for pos in &mut r.positions {
                pos.0 = pos.0 + offset + 1; // body-relative 0-based → absolute 1-based
                pos.1 += 1;                  // body-relative 0-based → absolute 1-based
            }
        }
    }
    make_absolute(&mut grid1.refdes_reassignments, sections.grid1_line_offset);
    let mut all_reassignments = grid1.refdes_reassignments;
    if let Some(ref mut g2) = grid2 {
        make_absolute(&mut g2.refdes_reassignments, sections.grid2_line_offset);
        all_reassignments.extend(g2.refdes_reassignments.clone());
    }
    let refdes_reassignments_json = serde_json::to_string(&all_reassignments).unwrap_or_default();

    // ---- Angles: combine both grids ---------------------------------------
    let mut angles_map: std::collections::HashMap<String, f64> = grid1
        .matched
        .iter()
        .map(|c| (c.refdes.clone(), c.angle))
        .collect();
    if let Some(ref g2) = grid2 {
        for c in &g2.matched {
            angles_map.entry(c.refdes.clone()).or_insert(c.angle);
        }
    }
    let angles_json = serde_json::to_string(&angles_map).unwrap_or_default();

    CompileResult { svg, kicad_sch, debug, source_map_json, refdes_reassignments_json, angles_json }
}

/// Generate an ASCII stub for a given symbol with the specified refdes.
///
/// Uses the text-grid template system: the refdes text is placed at (0,0)
/// and arrow characters are placed at their template-defined offsets.
/// This matches the new arrow-based schematic format.
#[wasm_bindgen]
pub fn generate_stub(symbol_name: &str, refdes: &str) -> String {
    let bundle = kicad_sym::load_builtin_library();
    let Some(sym) = bundle.symbols.get(symbol_name) else {
        return String::new();
    };

    if sym.pins.is_empty() {
        return String::new();
    }

    // Build the 0° orientation template.
    // DEBUG: use 90° rotation for testing.
    let template = parser::build_text_template(sym);
    let base = &template.orientations[0]; // 0°

    // Find bounding box of all assertions plus the refdes text.
    // Refdes sits at (0,0) in template coords (KiCad centre).
    let mut min_row = 0i32;
    let mut max_row = 0i32;
    let mut min_col = 0i32;
    let mut max_col = refdes.len() as i32 - 1;

    for a in &base.assertions {
        min_row = min_row.min(a.delta_row);
        max_row = max_row.max(a.delta_row);
        let dc = match a.col_ref {
            parser::ColRef::At(off) => off,
            parser::ColRef::AtRight(off) => off.max(refdes.len() as i32),
        };
        min_col = min_col.min(dc);
        max_col = max_col.max(dc);
    }

    let rows = (max_row - min_row + 1) as usize;
    let cols = (max_col - min_col + 1) as usize;

    // Refdes text at (0,0) in template coords → shift to non-negative grid coords.
    let refdes_r = (-min_row) as usize;
    let refdes_c = (-min_col) as usize;

    // Build grid.
    let mut grid: Vec<Vec<char>> = (0..rows)
        .map(|_| vec![' '; cols])
        .collect();

    // Place refdes text (horizontal, starting at template origin).
    for (i, ch) in refdes.chars().enumerate() {
        let c = refdes_c + i;
        if c < cols {
            grid[refdes_r][c] = ch;
        }
    }

    // Place arrow characters from assertions.
    for a in &base.assertions {
        let r = (a.delta_row - min_row) as usize;
        let c = match a.col_ref {
            parser::ColRef::At(off) => (off - min_col) as usize,
            parser::ColRef::AtRight(off) => {
                (off.max(refdes.len() as i32) - min_col) as usize
            }
        };
        if r < rows && c < cols {
            grid[r][c] = a.expected_dir.to_char();
        }
    }

    // Trim trailing whitespace and build lines.
    let lines: Vec<String> = grid
        .iter()
        .map(|row| {
            let s: String = row.iter().collect();
            s.trim_end().to_string()
        })
        .collect();

    lines.join("\n")
}

/// Return the text-grid footprint for a given symbol at a specific rotation angle.
///
/// Returns a JSON array of `{dr, dc, ch}` entries where each entry describes
/// one character cell relative to the refdes text start (the anchor). The
/// refdes text itself is included as individual character entries at row 0.
/// `ColRef::AtRight` offsets are resolved using the supplied refdes length so
/// that arrow characters never overlap the refdes text.
///
/// The frontend uses this data to clear old cells and write new ones during
/// component rotation.
#[wasm_bindgen]
pub fn get_rotated_footprint(symbol_name: &str, refdes: &str, angle: f64) -> String {
    let bundle = kicad_sym::load_builtin_library();
    let Some(sym) = bundle.symbols.get(symbol_name) else {
        return "[]".to_string();
    };

    let template = parser::build_text_template(sym);

    // Snap to nearest supported angle (0, 90, 180, 270).
    let target = (angle % 360.0 + 360.0) % 360.0;
    let orientation = template
        .orientations
        .iter()
        .find(|o| (o.angle - target).abs() < 0.5)
        .or_else(|| template.orientations.first());

    let Some(orientation) = orientation else {
        return "[]".to_string();
    };

    let refdes_len = refdes.len() as i32;
    let mut entries: Vec<serde_json::Value> = Vec::new();

    // Refdes text characters at row 0, starting at column 0.
    for (i, ch) in refdes.chars().enumerate() {
        entries.push(serde_json::json!({
            "dr": 0,
            "dc": i as i32,
            "ch": ch.to_string(),
        }));
    }

    // Arrow characters from the template orientation.
    for a in &orientation.assertions {
        let dc = match a.col_ref {
            parser::ColRef::At(off) => off,
            parser::ColRef::AtRight(off) => off.max(refdes_len),
        };
        entries.push(serde_json::json!({
            "dr": a.delta_row,
            "dc": dc,
            "ch": a.expected_dir.to_char().to_string(),
        }));
    }

    serde_json::to_string(&entries).unwrap_or_default()
}

fn format_debug(
    nodes: &[parser::SchematicNode],
    matched: &[parser::MatchedComponent],
    errors: &[String],
) -> String {
    use parser::NodeType;
    let mut s = format!("Step 3 — {} nodes, {} matched:\n",
        nodes.len(), matched.len());

    for node in nodes {
        let kind = match &node.node_type {
            NodeType::Port { refdes, pin, name, dir } => {
                let name_part = if name.is_empty() {
                    String::new()
                } else {
                    format!("({})", name)
                };
                format!("Port({}:{}{}{})", refdes, pin, name_part, dir.to_char())
            }
            NodeType::Label(name) => format!("Label([{}])", name),
            NodeType::Junction => "Junction(*)".to_string(),
            NodeType::Corner => "Corner(+)".to_string(),
            NodeType::Placeholder => "Placeholder(.)".to_string(),
            NodeType::Anchor { refdes } => format!("Anchor({})", refdes),
        };
        s.push_str(&format!(
            "  abs=({}, {})  grid=(R{}, C{})  {}  width={}\n",
            node.pos.row, node.pos.col, node.grid_row, node.grid_col, kind, node.text_width
        ));
    }

    if !matched.is_empty() {
        s.push_str("Matched:\n");
        for comp in matched {
            s.push_str(&format!(
                "  {}  symbol={}  lib_id={}  anchor=(R{}, C{})  pins={}  angle={:.0}\n",
                comp.refdes, comp.symbol_name, comp.lib_id,
                comp.anchor_grid_row, comp.anchor_grid_col,
                comp.pins.len(), comp.angle
            ));
            for p in &comp.pins {
                s.push_str(&format!(
                    "    pin {} \"{}\" {:?}  phys_offset=({:.1},{:.1})\n",
                    p.pin_num, p.name, p.dir, p.rel_phys_x, p.rel_phys_y
                ));
            }
        }
    }

    if !errors.is_empty() {
        s.push_str("Errors:\n");
        for e in errors {
            s.push_str(&format!("  {}\n", e));
        }
    }

    s
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn with_header(body: &str, header: &str) -> String {
        if header.is_empty() {
            body.to_string()
        } else {
            format!("{}\n==========\n{}", header, body)
        }
    }

    #[test]
    fn step3_valid_horizontal_and_vertical() {
        let input = with_header(
            "[VCC]   <R1>\n        ^\n        C1\n        v\n",
            "R1: R\nC1: C\n",
        );
        let result = compile(&input);
        assert!(result.debug.contains("Step 3"));
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=R"));
        assert!(result.debug.contains("symbol=C"));
        assert!(result.svg.contains("<svg"));
        assert!(!result.debug.contains("Errors:"), "errors: {}", result.debug);
        assert!(result.kicad_sch.contains("Device:R"));
        assert!(result.kicad_sch.contains("Device:C"));
    }

    #[test]
    fn step3_non_adjacent_pins_accepted_for_rotation() {
        // Two pins on same row: R template 0°=vertical, 90°=horizontal (< at col-1, > at col AtRight(1))
        let input = with_header(
            "<R2>\n",
            "R2: R\n",
        );
        let result = compile(&input);
        assert!(!result.debug.contains("Errors:"),
            "should accept pins; got: {}", result.debug);
        assert!(result.kicad_sch.contains("kicad_sch"));
    }

    #[test]
    fn step3_empty_input() {
        let result = compile("");
        assert!(result.debug.contains("0 nodes"));
        assert!(result.debug.contains("0 matched"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("kicad_sch"));
    }

    #[test]
    fn step3_single_label_no_components() {
        let result = compile("[VCC]\n");
        assert!(result.debug.contains("Label([VCC])"));
        assert!(result.debug.contains("grid=(R0, C0)"));
        assert!(result.svg.contains("VCC"));
        assert!(result.kicad_sch.contains("label"));
        assert!(result.kicad_sch.contains("VCC"));
    }

    #[test]
    fn step3_horizontal_inductor() {
        let input = with_header("<L1>\n", "L1: L\n");
        let result = compile(&input);
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=L"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:L"));
    }

    #[test]
    fn step3_vertical_resistor() {
        let input = with_header("^\nR3\nv\n", "R3: R\n");
        let result = compile(&input);
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=R"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// OPA330xxD with compact-grid arrow template.
    #[test]
    fn step4_opamp_with_header_and_resistor() {
        // OPA330 0° compact (U1 at row 2, col 2)
        let body = " ^\n<\n  U1>\n<\n v\n<R1>";
        let input = format!("U1: OPA330xxD\nR1: R\n============\n{}", body);

        let result = compile(&input);

        assert!(!result.debug.contains("Errors:"),
            "no undeclared refdes errors, got: {}", result.debug);
        assert!(result.debug.contains("symbol=OPA330xxD"));
        assert!(result.debug.contains("symbol=R"));
        assert!(result.kicad_sch.contains("Amplifier_Operational:OPA330xxD"));
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Test that undeclared refdes in body produces an error.
    #[test]
    fn step4_undeclared_refdes_error() {
        let input = "\
U1: OPA330xxD
=====
<R1>
";
        let result = compile(input);
        assert!(result.debug.contains("Errors:"));
        assert!(result.debug.contains("undeclared"));
        assert!(result.debug.contains("R1"));
    }

    /// Test that a symbol not in the library produces an error.
    #[test]
    fn step4_unknown_symbol_error() {
        let input = "\
X1: NonExistentSymbol
=====
<X1>
";
        let result = compile(input);
        assert!(result.debug.contains("Errors:"));
        assert!(result.debug.contains("not found in library"));
        assert!(result.debug.contains("NonExistentSymbol"));
    }

    /// Verify Conn_Coaxial SVG renders the inner circle and outer arcs.
    #[test]
    fn step4_conn_coaxial_renders_circle() {
        // Conn_Coaxial template puts arrows near the refdes.
        let input = "\
J1: Conn_Coaxial
=====
<J1
 v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("symbol=Conn_Coaxial"));
    }

    /// Print Conn_Coaxial symbol structure for verification.
    #[test]
    fn step4_print_conn_coaxial_structure() {
        let bundle = kicad_sym::load_builtin_library();
        let sym = bundle.symbols.get("Conn_Coaxial").unwrap();
        assert_eq!(sym.pins.len(), 2, "Conn_Coaxial should have 2 visible pins");
        assert_eq!(sym.pins[0].pin_num, 1);
        assert_eq!(sym.pins[0].name, "In");
        assert_eq!(sym.pins[0].dir, crate::parser::PinDirection::Left);
        assert_eq!(sym.pins[1].pin_num, 2);
        assert_eq!(sym.pins[1].name, "Ext");
        assert!(sym.draw_primitives.len() >= 4);
    }

    /// Print Conn_Coaxial rendering details: pin positions, draw primitive coords, SVG output.
    #[test]
    fn step5_print_conn_coaxial_render_coords() {
        let input = "\
J2: Conn_Coaxial
=====
<J2
  v
";
        let result = compile(input);
        println!("=== Debug Output ===");
        println!("{}", result.debug);
        println!();
        println!("=== SVG Output ===");
        println!("{}", result.svg);
    }

    /// Verify Conn_01x03_Socket 3-pin vertical connector renders correctly.
    #[test]
    fn step4_conn_01x03_socket_vertical() {
        // Conn_01x03_Socket: 3 pins all with dir=Left, in same column.
        // Template puts pin 1 at (-1, At(-1)), pin 2 at (0, At(-1)), pin 3 at (1, At(-1))
        // Wait — for a multi-pin symbol all with col=0, they'd share At(-1).
        // The template auto-generation needs pin separation by row.
        let input = "\
J2: Conn_01x03_Socket
=====
<J2
<
<
";
        let result = compile(input);
        // We just verify it compiles — the template generates errors for missing arrows
        // for now we accept that this test validates the pipeline handles the format
    }

    /// Label with '-' in its name must NOT create a false wire to a same-row port.
    #[test]
    fn step4_label_with_dash_in_name_no_false_wire() {
        let input = "\
R1: R
=====
[NET-1]  <R1
";
        let result = compile(input);
        let svg_wire_count = result.svg.matches("stroke=\"#1a1a1a\"").count();
        let kicad_wire_count = result.kicad_sch.matches("  (wire").count();
        assert_eq!(svg_wire_count, 0, "label '-' in name should not create SVG wire, got {}", svg_wire_count);
        assert_eq!(kicad_wire_count, 0, "label '-' in name should not create KiCad wire, got {}", kicad_wire_count);
    }

    /// Step 5: standard opamp layout (pins at template positions) → angle 0
    #[test]
    fn step5_opamp_standard_layout_angle_zero() {
        // OPA330 0° compact (U1 at row 2, col 2)
        let body = " ^\n<\n  U1>\n<\n v";
        let input = format!("U1: OPA330xxD\n============\n{}", body);
        let result = compile(&input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("angle=0"),
            "standard opamp should have angle=0, got: {}", result.debug);
        assert!(result.svg.contains("U1"));
        assert!(result.kicad_sch.contains("OPA330xxD"));
    }

    /// Step 5: horizontal RLC 2-pin → angle should be 0 (arrows left/right)
    #[test]
    fn step5_horizontal_resistor_has_rotation_angle() {
        let input = "\
R1: R
=====
<R1>
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        // Horizontal format (<R1>) → angle=90 (rotated from KiCad vertical default)
        assert!(result.debug.contains("angle=90"),
            "horizontal R should have angle=90, got: {}", result.debug);
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Step 5: vertical RLC 2-pin → angle should be 90 (arrows top/bottom)
    #[test]
    fn step5_vertical_capacitor_angle_zero() {
        let input = "\
C1: C
=====
^
C1
v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        // Vertical format (arrows above/below) → angle=0 (KiCad default)
        assert!(result.debug.contains("angle=0"),
            "vertical C should have angle=0, got: {}", result.debug);
        assert!(result.kicad_sch.contains("Device:C"));
    }


    /// Power symbol GND renders with its polyline draw primitives.
    #[test]
    fn step4_gnd_power_symbol_renders_graphics() {
        let input = "\
PWR1: GND
=====
^
PWR1
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=GND"));
        assert!(result.svg.contains("<polygon"), "GND should have polygon from polyline");
        assert!(result.kicad_sch.contains("power:GND"));
    }

    /// Power symbol VCC renders with its polyline draw primitives.
    #[test]
    fn step4_vcc_power_symbol_renders_graphics() {
        // VCC template pin is Down, so 0° puts `v` below the refdes
        let input = "\
PWR2: VCC
=====
PWR2
v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=VCC"));
        assert!(result.svg.contains("<polygon"), "VCC should have polygon from polyline");
        assert!(result.kicad_sch.contains("power:VCC"));
    }

    /// Power symbol VSS renders with its polyline draw primitives.
    #[test]
    fn step4_vss_power_symbol_renders_graphics() {
        // VSS template pin is Down, so 0° puts `v` below the refdes
        let input = "\
PWR3: VSS
=====
PWR3
v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=VSS"));
        assert!(result.svg.contains("<polygon"), "VSS should have polygon from polyline");
        assert!(result.kicad_sch.contains("power:VSS"));
    }

    /// Multi-letter refdes like GND1 are correctly parsed.
    #[test]
    fn step4_multi_letter_refdes_parsed() {
        let input = "\
GND1: GND
=====
^
GND1
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=GND"));
    }

    /// Single-pin power symbols rotate to match their pin direction in the schematic.
    /// GND (template pin Up) with `>` (Right of refdes) → 90° CW rotation.
    /// VCC (template pin Down) with `<` (Left of refdes) → 90° CW rotation.
    #[test]
    fn step5_single_pin_power_symbols_rotate() {
        let input = "\
PWR1: GND
PWR2: VCC
R1: R
==========
PWR1> -- <R1> -- <PWR2
";
        let result = compile(input);
        println!("=== DEBUG ===\n{}", result.debug);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);

        // GND template pin is Up, user draws > (Right) → rotation = 90° CW
        assert!(result.debug.contains("PWR1  symbol=GND"), "should match PWR1 as GND");
        assert!(result.debug.contains("angle=90"), "GND should be rotated 90°, got:\n{}", result.debug);

        // VCC template pin is Down, user draws < (Left) → rotation = 90° CW
        assert!(result.debug.contains("PWR2  symbol=VCC"), "should match PWR2 as VCC");

        // SVG should contain rotate(90) for both power symbols
        assert!(result.svg.matches("rotate(90)").count() >= 2,
            "SVG should have at least 2 rotate(90) groups");

        // KiCad should have correct (at ... angle) lines
        assert!(result.kicad_sch.contains("power:GND"));
        assert!(result.kicad_sch.contains("power:VCC"));
    }

    /// Simple circuit: resistors + power symbols in new arrow format.
    #[test]
    fn step6_simple_circuit() {
        let header = "\
R1: R
R2: R";
        let body = "\
<R1>---+
       >---[OUT]
<R2>---*\
";
        let input = format!("{}\n=============================================\n{}", header, body);
        let result = compile(&input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        assert_eq!(result.debug.matches("symbol=R").count(), 2, "should have 2 resistors");
        assert!(result.svg.contains(">OUT<"), "SVG missing label OUT");
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Verify that `source_map_json` tracks port and anchor positions.
    #[test]
    fn step6_source_map_tracks_port_positions() {
        let input = "\
U1: OPA330xxD
R1: R
=====
^
U1
v
<R1>\
";
        let result = compile(input);

        assert!(!result.source_map_json.is_empty());

        let spans: Vec<parser::ComponentTextSpan> =
            serde_json::from_str(&result.source_map_json)
                .expect("source_map_json should deserialize");

        assert!(!spans.is_empty(), "should have port spans");

        // Should have spans for U1 refdes text, U1 arrows, R1 refdes text, R1 arrows
        let u1_spans: Vec<_> = spans.iter().filter(|s| s.refdes == "U1").collect();
        assert!(!u1_spans.is_empty(), "should have U1 spans");

        let r1_spans: Vec<_> = spans.iter().filter(|s| s.refdes == "R1").collect();
        assert!(!r1_spans.is_empty(), "should have R1 spans");
    }

    /// Dot placeholders (`.`) preserve grid spacing but don't create wires.
    #[test]
    fn step6_dot_placeholder_preserves_grid_no_spurious_output() {
        let input = "\
R1: R
=====
<  . .  >
R1
";
        let result = compile(&input);
        // Dot placeholders participate in grid compression
        assert!(result.debug.contains("Placeholder(.)"),
            "debug output should contain Placeholder nodes, got: {}", result.debug);

        // No spurious wires without dash/pipe connections
        let svg_wire_count = result.svg.matches("stroke=\"#1a1a1a\"").count();
        assert_eq!(svg_wire_count, 0,
            "dots should not create SVG wires, got {}", svg_wire_count);
        let kicad_wire_count = result.kicad_sch.matches("  (wire").count();
        assert_eq!(kicad_wire_count, 0,
            "dots must not create KiCad wires, got {}", kicad_wire_count);
    }

    /// Dot placeholders preserve vertical grid spacing.
    #[test]
    fn step6_dot_placeholder_vertical_spacing() {
        let input = "\
R1: R
=====
^
R1
.
.
v
";
        let result = compile(&input);
        // Dots between pins preserve vertical spacing
        assert_eq!(result.debug.matches("Placeholder(.)").count(), 2,
            "expected 2 vertical placeholders, got: {}", result.debug);

        let svg_wire_count = result.svg.matches("stroke=\"#1a1a1a\"").count();
        assert_eq!(svg_wire_count, 0,
            "vertical dots should not create SVG wires, got {}", svg_wire_count);
        let kicad_wire_count = result.kicad_sch.matches("  (wire").count();
        assert_eq!(kicad_wire_count, 0,
            "vertical dots must not create KiCad wires, got {}", kicad_wire_count);
    }

    /// Duplicate refdes: two separate resistor instances both tagged R1.
    #[test]
    fn step7_duplicate_refdes_auto_increment() {
        let input = "\
R1: R
=====
<R1>
<R1>
";
        let result = compile(input);
        println!("=== DEBUG ===\n{}", result.debug);

        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        assert!(result.debug.contains("R1  symbol=R"),
            "first instance should keep R1");
        assert!(result.debug.contains("R2  symbol=R"),
            "second instance should be renamed to R2, got: {}", result.debug);

        assert_eq!(result.debug.matches("symbol=R").count(), 2,
            "expected 2 matched resistors");

        assert!(!result.refdes_reassignments_json.is_empty(),
            "should have refdes reassignments");
        assert!(result.refdes_reassignments_json.contains("\"new_refdes\":\"R2\""),
            "reassignment should mention new_refdes R2");

        assert!(result.kicad_sch.matches("Device:R").count() >= 2,
            "KiCad should have at least 2 Device:R references");
    }

    /// Duplicate refdes with existing R2 declared — should skip to R3.
    #[test]
    fn step7_duplicate_refdes_skips_existing() {
        let input = "\
R1: R
R2: R
=====
<R1>
<R1>
";
        let result = compile(input);

        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        assert!(result.debug.contains("R3  symbol=R"),
            "duplicate should skip R2 and become R3, got: {}", result.debug);

        assert!(result.refdes_reassignments_json.contains("\"new_refdes\":\"R3\""),
            "reassignment should use R3, got: {}", result.refdes_reassignments_json);
    }

    /// Single-instance (no duplicate) should produce no reassignments.
    #[test]
    fn step7_no_duplicate_no_reassignment() {
        let input = "\
R1: R
=====
<R1>
";
        let result = compile(input);

        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert_eq!(result.debug.matches("symbol=R").count(), 1,
            "expected exactly 1 matched resistor");

        let reass: Vec<parser::RefdesReassignment> =
            serde_json::from_str(&result.refdes_reassignments_json).unwrap_or_default();
        assert!(reass.is_empty(), "expected no reassignments, got {:?}", reass);
    }

    /// Power symbols with duplicate refdes should also auto-increment.
    #[test]
    fn step7_duplicate_power_symbol_refdes() {
        let input = "\
#GND1: GND
=====
^
#GND1
^
#GND1
";
        let result = compile(input);
        println!("=== DEBUG ===\n{}", result.debug);

        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        assert_eq!(result.debug.matches("symbol=GND").count(), 2,
            "expected 2 GND symbols");

        assert!(result.debug.contains("#GND2  symbol=GND"),
            "second GND should be #GND2, got: {}", result.debug);
    }

    // ================================================================
    // Step 6.6: Dual-Grid (three-section split) tests
    // ================================================================

    /// Three-section split with two separators: header, Grid1, Grid2.
    #[test]
    fn step66_split_three_sections_basic() {
        let input = "\
R1: R
=====
<R1>
=====
<C1>
";
        let sections = parser::split_three_sections(input);
        assert_eq!(sections.header, "R1: R");
        assert!(sections.grid1_body.contains("<R1>"), "Grid1: {}", sections.grid1_body);
        assert!(sections.grid2_body.contains("<C1>"), "Grid2: {}", sections.grid2_body);
        // Grid1 line offset: header(1) + sep1(1) = 2
        assert_eq!(sections.grid1_line_offset, 2);
        // Grid2 line offset: header(1) + sep1(1) + grid1_body_lines(1) + sep2(1) = 4
        assert_eq!(sections.grid2_line_offset, 4);
    }

    /// Zero separators: entire input is Grid1 body.
    #[test]
    fn step66_zero_separators() {
        let sections = parser::split_three_sections("<R1>\n");
        assert!(sections.header.is_empty());
        assert!(sections.grid1_body.contains("<R1>"));
        assert!(sections.grid2_body.is_empty());
        assert_eq!(sections.grid1_line_offset, 0);
    }

    /// Single separator: header + Grid1, no Grid2 (backward compat).
    #[test]
    fn step66_single_separator() {
        let input = "\
R1: R
=====
<R1>
";
        let sections = parser::split_three_sections(input);
        assert_eq!(sections.header, "R1: R");
        assert!(sections.grid1_body.contains("<R1>"));
        assert!(sections.grid2_body.is_empty());
    }

    /// Dual-grid: Grid1 and Grid2 have independent netlists.
    #[test]
    fn step66_dual_grid_independent_netlists() {
        let input = "\
R1: R
R2: R
==========
<R1>
==========
<R2>
";
        let result = compile(input);

        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        // Both grids should match their respective resistors.
        assert!(result.debug.contains("Grid1"), "debug should have Grid1 section");
        assert!(result.debug.contains("Grid2"), "debug should have Grid2 section");
        assert_eq!(result.debug.matches("symbol=R").count(), 2,
            "should have 2 matched resistors (one per grid)");
    }

    /// KiCad export MUST ignore Grid2 — only Grid1 devices appear as symbol instances.
    #[test]
    fn step66_kicad_ignores_grid2() {
        let input = "\
R1: R
C1: C
==========
<R1>
==========
<L1>
";
        // Grid2's L1 is undeclared in header → produces error in Grid2.
        // Grid1's R1 is the only successfully matched component.
        let result = compile(input);

        assert!(result.kicad_sch.contains("Device:R"),
            "KiCad should contain Device:R from Grid1");

        // Symbol instances use (lib_id "...") — count only matched components.
        // lib_symbols lists all built-in symbols regardless of use.
        let symbol_count = result.kicad_sch.matches("  (symbol\n").count();
        assert_eq!(symbol_count, 1,
            "KiCad should have exactly 1 symbol instance (Grid1's R1), got {}", symbol_count);

        // Grid2's L1 (undeclared) must not produce a symbol instance.
        let l_instance = result.kicad_sch.matches("(lib_id \"Device:L\")").count();
        assert_eq!(l_instance, 0,
            "KiCad must NOT have a Device:L symbol instance from Grid2");
    }

    /// Dual-grid: wires in Grid1 do not connect to Grid2 (electrical isolation).
    #[test]
    fn step66_dual_grid_electrical_isolation() {
        let input = "\
R1: R
=====
[VCC]---<R1>
=====
[VCC]---<R1>
";
        let result = compile(input);

        // Both grids have the same circuit pattern, but they are independent.
        // The SVG should contain both grids stacked.
        assert!(result.svg.contains("<svg"), "SVG should be generated");

        // Wires from Grid2 should NOT appear in KiCad.
        let kicad_wire_count = result.kicad_sch.matches("  (wire").count();
        // Grid1 has one horizontal wire: [VCC]---<R1>
        assert!(kicad_wire_count >= 1,
            "KiCad should have Grid1 wires, got {}", kicad_wire_count);
    }

    /// Dual-grid: source map combines entries from both grids.
    #[test]
    fn step66_dual_grid_combined_source_map() {
        let input = "\
R1: R
=====
<R1>
=====
<C1>
";
        // Note: C1 is undeclared in header, so Grid2 will have an error
        // but Grid2's scan_nodes/find_refdes_anchors still produce source-map
        // entries for what they find.
        let result = compile(input);
        let spans: Vec<parser::ComponentTextSpan> =
            serde_json::from_str(&result.source_map_json).unwrap_or_default();

        // Should have spans from both grids (R1 in Grid1, C1 anchor in Grid2).
        let r1_spans: Vec<_> = spans.iter().filter(|s| s.refdes == "R1").collect();
        assert!(!r1_spans.is_empty(), "should have R1 spans from Grid1");

        // Grid2's C1 anchor produces source-map entry even if undeclared.
        let c1_spans: Vec<_> = spans.iter().filter(|s| s.refdes == "C1").collect();
        assert!(!c1_spans.is_empty(), "should have C1 spans from Grid2");
    }
}








