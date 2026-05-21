use crate::{Circuit, NetEndpoint};
use uuid::Uuid;

// Layout constants (mm, KiCad internal units).
const Y_CENTER: f64 = 91.44;
const PIN_OFFSET: f64 = 3.81; // pin distance from symbol centre (matches lib_symbol pin at)
const LABEL_EXT: f64 = 6.35; // wire extension from pin tip to label
const SPACING: f64 = 12.7; // component-to-component centre spacing
const FIRST_X: f64 = 110.0; // x of the first net label

/// Project-level UUID — deterministic so output is stable across runs.
fn project_uuid() -> Uuid {
    Uuid::from_u128(0x721c986eb7c3470f8e92e3fd1d22244e)
}

/// Deterministic per-element UUID derived from a small counter.
/// Ensures valid v4 UUID format (version=4, variant=10xx).
fn make_uuid(n: u128) -> String {
    let mut bytes = n.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0F) | 0x40; // set version 4 nibble
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // set variant 10
    Uuid::from_bytes(bytes).to_string()
}

pub fn generate(circuit: &Circuit) -> String {
    let mut s = String::new();
    let proj_uuid = project_uuid().to_string();

    // ---- header ----------------------------------------------------------
    s.push_str("(kicad_sch\n");
    s.push_str("  (version 20260306)\n");
    s.push_str("  (generator \"texsch\")\n");
    s.push_str("  (generator_version \"0.1\")\n");
    s.push_str(&format!("  (uuid \"{}\")\n", proj_uuid));
    s.push_str("  (paper \"A4\")\n");

    // ---- lib_symbols -----------------------------------------------------
    emit_lib_symbols(&mut s);

    // ---- layout: walk the linear chain and collect positions -------------
    let mut comp_positions: Vec<(String, f64)> = Vec::new(); // (refdes, x_mm)
    let mut label_positions: Vec<(String, f64)> = Vec::new(); // (name, x_mm)

    layout_chain(circuit, &mut comp_positions, &mut label_positions);

    // ---- wires -----------------------------------------------------------
    for (i, seg) in circuit.connections.iter().enumerate() {
        let (x1, x2) = match (&seg.from, &seg.to) {
            (NetEndpoint::Label(name), NetEndpoint::ComponentPin { refdes, pin }) => {
                let comp_x = find_comp_x(&comp_positions, refdes);
                let pin_x = comp_x + pin_offset(*pin);
                let lbl_x = find_label_x(&label_positions, name);
                order(pin_x, lbl_x)
            }
            (NetEndpoint::ComponentPin { refdes, pin }, NetEndpoint::Label(name)) => {
                let comp_x = find_comp_x(&comp_positions, refdes);
                let pin_x = comp_x + pin_offset(*pin);
                let lbl_x = find_label_x(&label_positions, name);
                order(pin_x, lbl_x)
            }
            (NetEndpoint::ComponentPin { refdes: r1, pin: p1 },
             NetEndpoint::ComponentPin { refdes: r2, pin: p2 }) =>
            {
                let x1 = find_comp_x(&comp_positions, r1) + pin_offset(*p1);
                let x2 = find_comp_x(&comp_positions, r2) + pin_offset(*p2);
                order(x1, x2)
            }
            _ => continue,
        };

        s.push_str("  (wire\n");
        s.push_str(&format!(
            "    (pts (xy {:.2} {:.2}) (xy {:.2} {:.2}))\n",
            x1, Y_CENTER, x2, Y_CENTER
        ));
        s.push_str("    (stroke (width 0) (type default))\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(2000 + i as u128)));
        s.push_str("  )\n");
    }

    // ---- labels ----------------------------------------------------------
    for (i, (name, lx)) in label_positions.iter().enumerate() {
        s.push_str("  (label\n");
        s.push_str(&format!("    \"{}\"\n", name));
        s.push_str(&format!("    (at {:.2} {:.2} 0)\n", lx, Y_CENTER));
        s.push_str("    (effects\n");
        s.push_str("      (font (size 1.27 1.27))\n");
        s.push_str("      (justify left bottom)\n");
        s.push_str("    )\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(3000 + i as u128)));
        s.push_str("  )\n");
    }

    // ---- symbols (instances) ---------------------------------------------
    for (i, comp) in circuit.components.iter().enumerate() {
        let cx = find_comp_x(&comp_positions, &comp.refdes);

        s.push_str("  (symbol\n");
        s.push_str(&format!(
            "    (lib_id \"Device:{}\")\n",
            comp.comp_type.letter()
        ));
        s.push_str(&format!("    (at {:.2} {:.2} 90)\n", cx, Y_CENTER));
        s.push_str("    (unit 1)\n");
        s.push_str("    (body_style 1)\n");
        s.push_str("    (exclude_from_sim no)\n");
        s.push_str("    (in_bom yes)\n");
        s.push_str("    (on_board yes)\n");
        s.push_str("    (in_pos_files yes)\n");
        s.push_str("    (dnp no)\n");
        s.push_str("    (fields_autoplaced yes)\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(i as u128 * 2)));

        // Reference above the symbol
        s.push_str(&format!(
            "    (property \"Reference\" \"{}\"\n",
            comp.refdes
        ));
        s.push_str(&format!(
            "      (at {:.2} {:.2} 90)\n",
            cx, Y_CENTER - 6.35
        ));
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Value below Reference
        s.push_str(&format!(
            "    (property \"Value\" \"{}\"\n",
            comp.value
        ));
        s.push_str(&format!(
            "      (at {:.2} {:.2} 90)\n",
            cx, Y_CENTER - 3.81
        ));
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Footprint
        s.push_str("    (property \"Footprint\" \"\"\n");
        s.push_str(&format!("      (at {:.2} {:.2} 0)\n", cx + 3.81, Y_CENTER));
        s.push_str("      (hide yes)\n");
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Datasheet
        s.push_str("    (property \"Datasheet\" \"\"\n");
        s.push_str(&format!("      (at {:.2} {:.2} 0)\n", cx, Y_CENTER));
        s.push_str("      (hide yes)\n");
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Description
        let desc = match comp.comp_type {
            crate::CompType::Resistor => "Resistor",
            crate::CompType::Capacitor => "Unpolarized capacitor",
            crate::CompType::Inductor => "Inductor",
        };
        s.push_str(&format!(
            "    (property \"Description\" \"{}\"\n",
            desc
        ));
        s.push_str(&format!("      (at {:.2} {:.2} 0)\n", cx, Y_CENTER));
        s.push_str("      (hide yes)\n");
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Pins (reference-only — position is defined in lib_symbols)
        s.push_str(&format!(
            "    (pin \"1\" (uuid \"{}\"))\n",
            make_uuid(1000 + i as u128 * 2)
        ));
        s.push_str(&format!(
            "    (pin \"2\" (uuid \"{}\"))\n",
            make_uuid(1001 + i as u128 * 2)
        ));

        // instances
        s.push_str("    (instances\n");
        s.push_str(&format!("      (project \"\"\n"));
        s.push_str(&format!("        (path \"/{}\"\n", proj_uuid));
        s.push_str(&format!("          (reference \"{}\")\n", comp.refdes));
        s.push_str("          (unit 1)\n");
        s.push_str("        )\n");
        s.push_str("      )\n");
        s.push_str("    )\n");

        s.push_str("  )\n");
    }

    // ---- sheet_instances -------------------------------------------------
    s.push_str("  (sheet_instances\n");
    s.push_str("    (path \"/\" (page \"1\"))\n");
    s.push_str("  )\n");

    // ---- footer ----------------------------------------------------------
    s.push_str("  (embedded_fonts no)\n");
    s.push_str(")\n");

    s
}

// ---------------------------------------------------------------------------
// layout helpers
// ---------------------------------------------------------------------------

/// Walk the linear chain and compute x positions for components and labels.
fn layout_chain(
    circuit: &Circuit,
    comp_pos: &mut Vec<(String, f64)>,
    lbl_pos: &mut Vec<(String, f64)>,
) {
    let begin_x = FIRST_X;

    for seg in &circuit.connections {
        match (&seg.from, &seg.to) {
            // label → component-pin  (left side)
            (NetEndpoint::Label(name), NetEndpoint::ComponentPin { refdes, .. }) => {
                lbl_pos.push((name.clone(), begin_x));
                let cx = begin_x + LABEL_EXT + PIN_OFFSET;
                comp_pos.push((refdes.clone(), cx));
            }
            // component-pin → label  (right side)
            (NetEndpoint::ComponentPin { refdes, .. }, NetEndpoint::Label(name)) => {
                if comp_pos.is_empty() {
                    let cx = begin_x + PIN_OFFSET;
                    comp_pos.push((refdes.clone(), cx));
                }
                let last_cx = last_comp_x(comp_pos);
                let lx = last_cx + PIN_OFFSET + LABEL_EXT;
                lbl_pos.push((name.clone(), lx));
            }
            // component-pin → component-pin  (between two components)
            (NetEndpoint::ComponentPin { refdes: r1, .. },
             NetEndpoint::ComponentPin { refdes: r2, .. }) =>
            {
                if !comp_pos.iter().any(|(r, _)| r == r1) {
                    let cx = if comp_pos.is_empty() {
                        begin_x + LABEL_EXT + PIN_OFFSET
                    } else {
                        last_comp_x(comp_pos) + SPACING
                    };
                    comp_pos.push((r1.clone(), cx));
                }
                if !comp_pos.iter().any(|(r, _)| r == r2) {
                    let cx = last_comp_x(comp_pos) + SPACING;
                    comp_pos.push((r2.clone(), cx));
                }
            }
            _ => {}
        }
    }
}

fn find_comp_x(positions: &[(String, f64)], refdes: &str) -> f64 {
    positions
        .iter()
        .find(|(r, _)| r == refdes)
        .map(|(_, x)| *x)
        .unwrap_or(FIRST_X)
}

fn find_label_x(positions: &[(String, f64)], name: &str) -> f64 {
    positions
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, x)| *x)
        .unwrap_or(FIRST_X)
}

fn last_comp_x(positions: &[(String, f64)]) -> f64 {
    positions.last().map(|(_, x)| *x).unwrap_or(FIRST_X)
}

fn order(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

/// At rotation 90, pin 0 (left) is at -PIN_OFFSET, pin 1 (right) at +PIN_OFFSET.
fn pin_offset(pin: usize) -> f64 {
    if pin == 0 { -PIN_OFFSET } else { PIN_OFFSET }
}

// ---------------------------------------------------------------------------
// library symbol definitions  (matching reference format)
// ---------------------------------------------------------------------------

fn emit_lib_symbols(s: &mut String) {
    s.push_str("  (lib_symbols\n");

    // ---- Device:R --------------------------------------------------------
    s.push_str("    (symbol \"Device:R\"\n");
    s.push_str("      (pin_numbers (hide yes))\n");
    s.push_str("      (pin_names (offset 0))\n");
    s.push_str("      (exclude_from_sim no)\n");
    s.push_str("      (in_bom yes)\n");
    s.push_str("      (on_board yes)\n");
    s.push_str("      (in_pos_files yes)\n");
    s.push_str("      (duplicate_pin_numbers_are_jumpers no)\n");

    emit_lib_prop(s, "Reference", "R", 2.032, 0.0, 90, false);
    emit_lib_prop(s, "Value", "R", 0.0, 0.0, 90, false);
    emit_lib_prop_hidden(s, "Footprint", "", -1.778, 0.0, 90);
    emit_lib_prop_hidden(s, "Datasheet", "", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "Description", "Resistor", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_keywords", "R res resistor", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_fp_filters", "R_*", 0.0, 0.0, 0);

    // R body: rectangle
    s.push_str("      (symbol \"R_0_1\"\n");
    s.push_str("        (rectangle\n");
    s.push_str("          (start -1.016 -2.54)\n");
    s.push_str("          (end 1.016 2.54)\n");
    s.push_str("          (stroke (width 0.254) (type default))\n");
    s.push_str("          (fill (type none))\n");
    s.push_str("        )\n");
    s.push_str("      )\n");

    // R pins
    s.push_str("      (symbol \"R_1_1\"\n");
    emit_pin(s, "passive", "line", 0.0, 3.81, 270, 1.27, "1");
    emit_pin(s, "passive", "line", 0.0, -3.81, 90, 1.27, "2");
    s.push_str("      )\n");
    s.push_str("      (embedded_fonts no)\n");
    s.push_str("    )\n");

    // ---- Device:C --------------------------------------------------------
    s.push_str("    (symbol \"Device:C\"\n");
    s.push_str("      (pin_numbers (hide yes))\n");
    s.push_str("      (pin_names (offset 0.254))\n");
    s.push_str("      (exclude_from_sim no)\n");
    s.push_str("      (in_bom yes)\n");
    s.push_str("      (on_board yes)\n");
    s.push_str("      (in_pos_files yes)\n");
    s.push_str("      (duplicate_pin_numbers_are_jumpers no)\n");

    emit_lib_prop(s, "Reference", "C", 0.635, 2.54, 0, false);
    emit_lib_prop(s, "Value", "C", 0.635, -2.54, 0, false);
    emit_lib_prop_hidden(s, "Footprint", "", 0.9652, -3.81, 0);
    emit_lib_prop_hidden(s, "Datasheet", "", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "Description", "Unpolarized capacitor", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_keywords", "cap capacitor", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_fp_filters", "C_*", 0.0, 0.0, 0);

    // C body: two horizontal polylines (plates at rotation 0)
    s.push_str("      (symbol \"C_0_1\"\n");
    // Top plate
    s.push_str("        (polyline\n");
    s.push_str("          (pts (xy -2.032 0.762) (xy 2.032 0.762))\n");
    s.push_str("          (stroke (width 0.508) (type default))\n");
    s.push_str("          (fill (type none))\n");
    s.push_str("        )\n");
    // Bottom plate
    s.push_str("        (polyline\n");
    s.push_str("          (pts (xy -2.032 -0.762) (xy 2.032 -0.762))\n");
    s.push_str("          (stroke (width 0.508) (type default))\n");
    s.push_str("          (fill (type none))\n");
    s.push_str("        )\n");
    s.push_str("      )\n");

    // C pins
    s.push_str("      (symbol \"C_1_1\"\n");
    emit_pin(s, "passive", "line", 0.0, 3.81, 270, 2.794, "1");
    emit_pin(s, "passive", "line", 0.0, -3.81, 90, 2.794, "2");
    s.push_str("      )\n");
    s.push_str("      (embedded_fonts no)\n");
    s.push_str("    )\n");

    // ---- Device:L --------------------------------------------------------
    s.push_str("    (symbol \"Device:L\"\n");
    s.push_str("      (pin_numbers (hide yes))\n");
    s.push_str("      (pin_names (offset 0))\n");
    s.push_str("      (exclude_from_sim no)\n");
    s.push_str("      (in_bom yes)\n");
    s.push_str("      (on_board yes)\n");
    s.push_str("      (in_pos_files yes)\n");
    s.push_str("      (duplicate_pin_numbers_are_jumpers no)\n");

    emit_lib_prop(s, "Reference", "L", 2.032, 0.0, 90, false);
    emit_lib_prop(s, "Value", "L", 0.0, 0.0, 90, false);
    emit_lib_prop_hidden(s, "Footprint", "", -1.778, 0.0, 90);
    emit_lib_prop_hidden(s, "Datasheet", "", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "Description", "Inductor", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_keywords", "L inductor coil", 0.0, 0.0, 0);
    emit_lib_prop_hidden(s, "ki_fp_filters", "L_*", 0.0, 0.0, 0);

    // L body: 4 arcs forming a coil
    s.push_str("      (symbol \"L_0_1\"\n");
    emit_arc(s, 0.0, 2.54, 1.016, 2.032, 0.0, 1.27);
    emit_arc(s, 0.0, 1.27, 1.016, 0.508, 0.0, 0.0);
    emit_arc(s, 0.0, 0.0, -1.016, -0.508, 0.0, -1.27);
    emit_arc(s, 0.0, -1.27, -1.016, -2.032, 0.0, -2.54);
    s.push_str("      )\n");

    // L pins
    s.push_str("      (symbol \"L_1_1\"\n");
    emit_pin(s, "passive", "line", 0.0, 3.81, 270, 1.27, "1");
    emit_pin(s, "passive", "line", 0.0, -3.81, 90, 1.27, "2");
    s.push_str("      )\n");
    s.push_str("      (embedded_fonts no)\n");
    s.push_str("    )\n");

    s.push_str("  )\n");
}

fn emit_lib_prop(s: &mut String, name: &str, value: &str, x: f64, y: f64, rot: i32, hide: bool) {
    s.push_str(&format!(
        "      (property \"{}\" \"{}\"\n",
        name, value
    ));
    s.push_str(&format!("        (at {:.3} {:.3} {})\n", x, y, rot));
    s.push_str("        (show_name no)\n");
    s.push_str("        (do_not_autoplace no)\n");
    if hide {
        s.push_str("        (hide yes)\n");
    }
    s.push_str("        (effects (font (size 1.27 1.27))");
    if name == "Reference" || name == "Value" {
        s.push_str(" (justify left)");
    }
    s.push_str(")\n");
    s.push_str("      )\n");
}

fn emit_lib_prop_hidden(s: &mut String, name: &str, value: &str, x: f64, y: f64, rot: i32) {
    emit_lib_prop(s, name, value, x, y, rot, true);
}

fn emit_pin(s: &mut String, etype: &str, shape: &str, x: f64, y: f64, dir: i32, len: f64, num: &str) {
    s.push_str(&format!(
        "        (pin {} {} (at {:.3} {:.3} {}) (length {:.3})\n",
        etype, shape, x, y, dir, len
    ));
    s.push_str("          (name \"\" (effects (font (size 1.27 1.27))))\n");
    s.push_str(&format!(
        "          (number \"{}\" (effects (font (size 1.27 1.27))))\n",
        num
    ));
    s.push_str("        )\n");
}

fn emit_arc(s: &mut String, sx: f64, sy: f64, mx: f64, my: f64, ex: f64, ey: f64) {
    s.push_str("        (arc\n");
    s.push_str(&format!(
        "          (start {:.3} {:.3}) (mid {:.3} {:.3}) (end {:.3} {:.3})\n",
        sx, sy, mx, my, ex, ey
    ));
    s.push_str("          (stroke (width 0.254) (type default))\n");
    s.push_str("          (fill (type none))\n");
    s.push_str("        )\n");
}
