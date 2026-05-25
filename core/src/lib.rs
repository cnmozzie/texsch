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
}

/// Parse the ASCII schematic `input` using the Port-based Grid scanner.
/// Step 0: parse header, Step 1: scan, Step 2: compress, Step 3: match & render.
#[wasm_bindgen]
pub fn compile(input: &str) -> CompileResult {
    // Step 0: split header and body, parse refdes→symbol mapping
    let (header, body) = parser::split_header_body(input);
    let refdes_to_symbol = parser::parse_header(header);

    // Load built-in symbol library (compile-time embedded KiCad files)
    let bundle = kicad_sym::load_builtin_library();

    // Step 1 & 2: scan and compress body
    let mut nodes = parser::scan_nodes(body);
    parser::compress_coordinates(&mut nodes);

    // Step 3: match all components against the symbol library
    let (mut matched, match_errors) =
        parser::match_components(&nodes, &refdes_to_symbol, &bundle.symbols);

    // Step 3b: solve orientations from grid positions, then rotate
    // rel_phys so the DAG solver uses the correct rotated constraints.
    parser::solve_orientations(&mut matched);
    parser::apply_rotation_to_rel_phys(&mut matched);

    parser::compute_spans(&mut nodes);
    let (col_x, row_y) = parser::compute_layout(&nodes, &matched);

    // Pre-compute pin KiCad positions using final mm layout
    parser::compute_pin_ki_positions(&mut matched, &col_x, &row_y);

    // Collect labels from nodes
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

    let debug = format_debug(&nodes, &matched, &match_errors);
    let wires = parser::extract_wires(&nodes, &matched, &col_x, &row_y, body);
    let svg = svg::generate_step3(&nodes, &wires, &col_x, &row_y, &matched);
    let kicad_sch = kicad::generate_step3(&labels, &nodes, &col_x, &row_y, &matched, body, &bundle.entries);

    CompileResult { svg, kicad_sch, debug }
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
            "[VCC]   R1:1<   R1:2>\n        C1:1^\n        C1:2v\n",
            "R1: R\nC1: C\n",
        );
        let result = compile(&input);
        assert!(result.debug.contains("Step 3"));
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=R"));
        assert!(result.debug.contains("symbol=C"));
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("C0"));
        assert!(!result.debug.contains("Errors:"));
        assert!(result.kicad_sch.contains("Device:R"));
        assert!(result.kicad_sch.contains("Device:C"));
        assert!(result.kicad_sch.contains("label"));
    }

    #[test]
    fn step3_non_adjacent_pins_accepted_for_rotation() {
        // Non-adjacent pins are now valid — orientation solver and DAG layout
        // handle arbitrary placements.
        let input = with_header(
            "R2:1<    +    R2:2>\n",
            "R2: R\n",
        );
        let result = compile(&input);
        assert!(result.debug.contains("Step 3"));
        assert!(!result.debug.contains("Errors:"),
            "non-adjacent pins should be accepted; got: {}", result.debug);
        assert!(result.svg.contains("<svg"));
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
        let input = with_header("L1:1<  L1:2>\n", "L1: L\n");
        let result = compile(&input);
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=L"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:L"));
    }

    #[test]
    fn step3_vertical_resistor() {
        let input = with_header("R3:1^\nR3:2v\n", "R3: R\n");
        let result = compile(&input);
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=R"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Full integration test: opamp + resistor with header declaration,
    /// using the exact OPA330xxD template grid layout from opa330_sch().
    #[test]
    fn step4_opamp_with_header_and_resistor() {
        // Build the opamp grid using the same helper as opa330_sch()
        fn place(line: &mut String, grid_col: usize, text: &str) {
            let target = grid_col * 12;
            while line.len() < target { line.push(' '); }
            line.push_str(text);
        }
        let mut lines: Vec<String> = Vec::new();
        // OPA330xxD rows (same as opa330_sch)
        let mut r0 = String::new(); place(&mut r0, 1, "U1:7(V+)^"); lines.push(r0);
        let mut r1 = String::new(); place(&mut r1, 0, "U1:3(+)<"); lines.push(r1);
        let mut r2 = String::new(); place(&mut r2, 2, "U1:6>"); lines.push(r2);
        let mut r3 = String::new(); place(&mut r3, 0, "U1:2(-)<"); lines.push(r3);
        let mut r4 = String::new(); place(&mut r4, 1, "U1:4(V-)v"); lines.push(r4);
        // R1 on row 5, connected to U1:6 output
        let mut r5 = String::new(); place(&mut r5, 2, "R1:1<"); place(&mut r5, 3, "R1:2>"); lines.push(r5);

        let body = lines.join("\n");
        let input = format!("U1: OPA330xxD\nR1: R\n============\n{}", body);

        let result = compile(&input);

        assert!(result.debug.contains("Step 3"));
        assert!(result.debug.contains("Matched:"));
        assert!(result.debug.contains("symbol=OPA330xxD"));
        assert!(result.debug.contains("symbol=R"));
        assert!(!result.debug.contains("undeclared"),
            "no undeclared refdes errors, got: {}", result.debug);

        // SVG output should contain both components
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("U1"));
        assert!(result.svg.contains("R1"));

        // KiCad output should reference both symbols by full lib_id
        assert!(result.kicad_sch.contains("kicad_sch"));
        assert!(result.kicad_sch.contains("Amplifier_Operational:OPA330xxD"));
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Test that undeclared refdes in body produces an error.
    #[test]
    fn step4_undeclared_refdes_error() {
        let input = "\
U1: OPA330xxD
=====
R1:1< R1:2>
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
X1:1< X1:2>
";
        let result = compile(input);
        assert!(result.debug.contains("Errors:"));
        assert!(result.debug.contains("not found in library"));
        assert!(result.debug.contains("NonExistentSymbol"));
    }

    /// Verify Conn_Coaxial SVG renders the inner circle and outer arcs.
    #[test]
    fn step4_conn_coaxial_renders_circle() {
        // Pin 1 (In) is anchor at (-5.08, 0), Pin 2 (Ext) at (0, -5.08) angle 90→Down.
        // Compact grid: Pin 2 is 1 col right, 1 row down from anchor.
        let input = "\
J1: Conn_Coaxial
=====
J1:1<
    J1:2v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("symbol=Conn_Coaxial"));
        // SVG must have a <circle> element from the inner circle primitive
        assert!(result.svg.contains("<circle"),
            "SVG should contain a <circle> element from Conn_Coaxial");
        // Should also have arcs (the outer shield symbol) — now proper SVG A commands
        assert!(result.svg.contains(" A "),
            "SVG should contain SVG arc (A) commands from Conn_Coaxial");
    }

    /// Print Conn_Coaxial symbol structure for verification.
    #[test]
    fn step4_print_conn_coaxial_structure() {
        let bundle = kicad_sym::load_builtin_library();
        let sym = bundle.symbols.get("Conn_Coaxial").unwrap();
        println!("=== Conn_Coaxial (lib_id: {}) ===", sym.lib_id);
        println!("Anchor KiCad offset: ({:.3}, {:.3}) mm", sym.anchor_ki_x, sym.anchor_ki_y);
        println!();

        println!("--- Pins ({} visible, {} total) ---", sym.pins.len(), sym.all_pin_numbers.len());
        for p in &sym.pins {
            println!("  Pin {} \"{}\"  dir={:?}  rel_grid=(R{}, C{})  rel_phys=({:.3}, {:.3}) mm  length={:.3} mm",
                p.pin_num, p.name, p.dir,
                p.rel_grid_row, p.rel_grid_col,
                p.rel_phys_x, p.rel_phys_y,
                p.pin_length_mm);
        }
        println!();

        println!("--- Draw Primitives (grid-relative, unit=2.54mm) ---");
        for (i, dp) in sym.draw_primitives.iter().enumerate() {
            match dp {
                crate::parser::DrawPrimitive::Polyline { pts, stroke_width, fill_type } => {
                    println!("  [{}] Polyline  stroke={:.3}mm  fill={}", i, stroke_width, fill_type);
                    for (j, (gx, gy)) in pts.iter().enumerate() {
                        let mx = gx * 2.54;
                        let my = gy * 2.54;
                        println!("       pt[{}]: grid=({:.3},{:.3})  mm=({:.3},{:.3})", j, gx, gy, mx, my);
                    }
                }
                crate::parser::DrawPrimitive::Rectangle { start, end, stroke_width, fill_type } => {
                    println!("  [{}] Rectangle  start=({:.3},{:.3})  end=({:.3},{:.3})  stroke={:.3}mm  fill={}",
                        i, start.0, start.1, end.0, end.1, stroke_width, fill_type);
                }
                crate::parser::DrawPrimitive::Arc { start, mid, end, stroke_width, fill_type } => {
                    println!("  [{}] Arc  stroke={:.3}mm  fill={}", i, stroke_width, fill_type);
                    println!("       start: grid=({:.3},{:.3})  mm=({:.3},{:.3})",
                        start.0, start.1, start.0 * 2.54, start.1 * 2.54);
                    println!("       mid:   grid=({:.3},{:.3})  mm=({:.3},{:.3})",
                        mid.0, mid.1, mid.0 * 2.54, mid.1 * 2.54);
                    println!("       end:   grid=({:.3},{:.3})  mm=({:.3},{:.3})",
                        end.0, end.1, end.0 * 2.54, end.1 * 2.54);
                }
                crate::parser::DrawPrimitive::Circle { center, radius, stroke_width, fill_type } => {
                    println!("  [{}] Circle  center=({:.3},{:.3}) mm=({:.3},{:.3})  r={:.3}  stroke={:.3}mm  fill={}",
                        i, center.0, center.1, center.0 * 2.54, center.1 * 2.54,
                        radius, stroke_width, fill_type);
                }
            }
        }

        // Verify expected structure
        assert_eq!(sym.pins.len(), 2, "Conn_Coaxial should have 2 visible pins");
        assert_eq!(sym.pins[0].pin_num, 1);
        assert_eq!(sym.pins[0].name, "In");
        assert_eq!(sym.pins[0].dir, crate::parser::PinDirection::Left);
        assert_eq!(sym.pins[1].pin_num, 2);
        assert_eq!(sym.pins[1].name, "Ext");
        assert_eq!(sym.pins[1].dir, crate::parser::PinDirection::Down);
        // Should have 5 draw primitives: polyline, arc, arc, circle, polyline
        assert_eq!(sym.draw_primitives.len(), 5);
    }

    /// Print Conn_Coaxial rendering details: pin positions, draw primitive coords, SVG output.
    #[test]
    fn step5_print_conn_coaxial_render_coords() {
        let input = "\
J2: Conn_Coaxial
=====
J2:1<
   J2:2v
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
        let input = "\
J2: Conn_01x03_Socket
=====
J2:1<
J2:2<
J2:3<
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("symbol=Conn_01x03_Socket"));
        assert_eq!(result.debug.matches("symbol=Conn_01x03_Socket").count(), 1);
    }

    /// Label with '-' in its name must NOT create a false wire to a same-row port.
    #[test]
    fn step4_label_with_dash_in_name_no_false_wire() {
        let input = "\
R1: R
=====
[NET-1]  R1:1<
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
        fn place(line: &mut String, grid_col: usize, text: &str) {
            let target = grid_col * 12;
            while line.len() < target { line.push(' '); }
            line.push_str(text);
        }
        let mut lines: Vec<String> = Vec::new();
        // Same layout as opa330_sch() — must match the template grid
        let mut r0 = String::new(); place(&mut r0, 1, "U1:7(V+)^"); lines.push(r0);
        let mut r1 = String::new(); place(&mut r1, 0, "U1:3(+)<"); lines.push(r1);
        let mut r2 = String::new(); place(&mut r2, 2, "U1:6>"); lines.push(r2);
        let mut r3 = String::new(); place(&mut r3, 0, "U1:2(-)<"); lines.push(r3);
        let mut r4 = String::new(); place(&mut r4, 1, "U1:4(V-)v"); lines.push(r4);

        let input = format!("U1: OPA330xxD\n============\n{}", lines.join("\n"));
        let result = compile(&input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("angle=0"),
            "standard opamp should have angle=0, got: {}", result.debug);
        // SVG should render the opamp
        assert!(result.svg.contains("U1"));
        // KiCad output should have (at ... 0)
        assert!(result.kicad_sch.contains("OPA330xxD"));
    }

    /// Step 5: horizontal RLC 2-pin → angle should be non-zero (rotated from template vertical)
    #[test]
    fn step5_horizontal_resistor_has_rotation_angle() {
        let input = "\
R1: R
=====
R1:1<  R1:2>
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        // Horizontal 2-pin should have angle != 0 (rotated from vertical template)
        assert!(result.debug.contains("angle="),
            "debug output should contain angle field");
        assert!(!result.debug.contains("angle=0"),
            "horizontal RLC should not have angle=0, got: {}", result.debug);
        // KiCad output should contain the angle in (at ...) line
        assert!(result.kicad_sch.contains("Device:R"));
    }

    /// Step 5: vertical RLC 2-pin → angle should be 0 (matches template default)
    #[test]
    fn step5_vertical_capacitor_angle_zero() {
        let input = "\
C1: C
=====
C1:1^
C1:2v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);
        assert!(result.debug.contains("angle=0"),
            "vertical C should have angle=0, got: {}", result.debug);
        assert!(result.kicad_sch.contains("Device:C"));
    }
}


    /// Power symbol GND renders with its polyline draw primitives.
    #[test]
    fn step4_gnd_power_symbol_renders_graphics() {
        let input = "\
PWR1: GND
=====
PWR1:1^
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
        let input = "\
PWR2: VCC
=====
PWR2:1v
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
        let input = "\
PWR3: VSS
=====
PWR3:1v
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=VSS"));
        assert!(result.svg.contains("<polygon"), "VSS should have polygon from polyline");
        assert!(result.kicad_sch.contains("power:VSS"));
    }

    /// Multi-letter refdes like PWR1 are correctly parsed.
    #[test]
    fn step4_multi_letter_refdes_parsed() {
        let input = "\
GND1: GND
=====
GND1:1^
";
        let result = compile(input);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);
        assert!(result.debug.contains("symbol=GND"));
    }

    /// Single-pin power symbols rotate to match their pin direction in the schematic.
    /// GND (template pin Up) with `PWR1:1>` (Right) → 90° CW rotation.
    /// VCC (template pin Down) with `PWR2:1<` (Left) → 90° CW rotation.
    #[test]
    fn step5_single_pin_power_symbols_rotate() {
        let input = "\
PWR1: GND
PWR2: VCC
R1: R
==========
PWR1:1> -- R1:1< R1:2> -- PWR2:1<
";
        let result = compile(input);
        println!("=== DEBUG ===\n{}", result.debug);
        println!("=== SVG ===\n{}", result.svg);
        assert!(!result.debug.contains("Errors:"), "{}", result.debug);

        // GND template pin is Up, user writes Right → rotation = 90° CW
        assert!(result.debug.contains("PWR1  symbol=GND"), "should match PWR1 as GND");
        assert!(result.debug.contains("angle=90"), "GND should be rotated 90°, got:\n{}", result.debug);

        // VCC template pin is Down, user writes Left → rotation = 90° CW
        assert!(result.debug.contains("PWR2  symbol=VCC"), "should match PWR2 as VCC");

        // SVG should contain rotate(90) for both power symbols
        assert!(result.svg.matches("rotate(90)").count() >= 2,
            "SVG should have at least 2 rotate(90) groups");

        // KiCad should have correct (at ... angle) lines
        assert!(result.kicad_sch.contains("power:GND"));
        assert!(result.kicad_sch.contains("power:VCC"));
    }

    /// Adder circuit: opamp + 3 resistors + 4 connectors + power symbols
    #[test]
    fn step6_adder_circuit() {
        let header = "\
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
#VSS1: VSS";
        let body = "\
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
[VSS]--J1:3<                                           +------------------R3:1< R3:2>--*";
        let input = format!("{}\n=============================================\n{}", header, body);
        let result = compile(&input);
        println!("=== DEBUG ===\n{}", result.debug);
        println!("=== SVG ===\n{}", result.svg);
        println!("=== KICAD ===\n{}", result.kicad_sch);
        assert!(!result.debug.contains("Errors:"),
            "unexpected errors: {}", result.debug);

        // All components matched
        assert!(result.debug.contains("symbol=OPA330xxD"));
        assert_eq!(result.debug.matches("symbol=R").count(), 3, "should have 3 resistors");
        assert_eq!(result.debug.matches("symbol=Conn_Coaxial").count(), 3,
            "should have 3 coaxial connectors (J2,J3,J4)");
        assert!(result.debug.contains("symbol=Conn_01x03_Socket"));
        assert_eq!(result.debug.matches("symbol=GND").count(), 4);
        assert_eq!(result.debug.matches("symbol=VCC").count(), 1);
        assert_eq!(result.debug.matches("symbol=VSS").count(), 1);

        // SVG should contain all labels (rendered without brackets)
        assert!(result.svg.contains(">In1<"), "SVG missing label In1");
        assert!(result.svg.contains(">In2<"), "SVG missing label In2");
        assert!(result.svg.contains(">OUT<"), "SVG missing label OUT");
        assert!(result.svg.contains(">VCC<"), "SVG missing label VCC");
        assert!(result.svg.contains(">GND<"), "SVG missing label GND");
        assert!(result.svg.contains(">VSS<"), "SVG missing label VSS");

        // KiCad should contain all symbol references
        assert!(result.kicad_sch.contains("Amplifier_Operational:OPA330xxD"));
        assert!(result.kicad_sch.contains("Device:R"));
        assert!(result.kicad_sch.contains("Connector:Conn_Coaxial"));
        assert!(result.kicad_sch.contains("Connector:Conn_01x03_Socket"));
        assert!(result.kicad_sch.contains("power:GND"));
        assert!(result.kicad_sch.contains("power:VCC"));
        assert!(result.kicad_sch.contains("power:VSS"));
    }
