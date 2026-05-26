use crate::kicad_sym::LibraryEntry;
use crate::parser::{MatchedComponent, NodeType, SchematicNode};
use uuid::Uuid;

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

fn emit_lib_symbol_from_file(s: &mut String, raw: &str, lib_prefix: &str, sym_name: &str) {
    let lib_id = format!("{}:{}", lib_prefix, sym_name);
    let search = format!("(symbol \"{}\"", sym_name);
    let sym_start = raw.find(&search).unwrap();
    let inner = &raw[sym_start..];

    // Find matching closing paren by counting nesting depth
    let mut depth = 0i32;
    let mut sym_end = 0;
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    sym_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let mut sym_text: String = chars[..sym_end].iter().collect();

    // Rename to fully-qualified lib_id
    sym_text = sym_text.replacen(&search, &format!("(symbol \"{}\"", lib_id), 1);

    // Fix Reference property — shorten value to its first character.
    // Find `(property "Reference" "…")` and replace "…" with its first char.
    if let Some(prop_pos) = sym_text.find("(property \"Reference\"") {
        let after = &sym_text[prop_pos..];
        if let Some(v0) = after.find('"') {
            let after_first = &after[v0 + 1..];
            if let Some(v1) = after_first.find('"') {
                let v2 = after_first[..v1].find('"');
                let val_end = v2.unwrap_or(v1);
                let old_val = &after_first[..val_end];
                if !old_val.is_empty() {
                    let ref_val = &old_val[0..1]; // first character
                    if old_val != ref_val {
                        let abs_start = prop_pos + v0 + 1;
                        let abs_end = abs_start + val_end;
                        let mut new_text = sym_text[..abs_start].to_string();
                        new_text.push_str(ref_val);
                        new_text.push_str(&sym_text[abs_end..]);
                        sym_text = new_text;
                    }
                }
            }
        }
    }

    // Fix Value property the same way (first-char shortening)
    if let Some(prop_pos) = sym_text.find("(property \"Value\"") {
        let after = &sym_text[prop_pos..];
        if let Some(v0) = after.find('"') {
            let after_first = &after[v0 + 1..];
            if let Some(v1) = after_first.find('"') {
                let v2 = after_first[..v1].find('"');
                let val_end = v2.unwrap_or(v1);
                let old_val = &after_first[..val_end];
                if !old_val.is_empty() {
                    let first_char = &old_val[0..1];
                    if old_val != first_char {
                        let abs_start = prop_pos + v0 + 1;
                        let abs_end = abs_start + val_end;
                        let mut new_text = sym_text[..abs_start].to_string();
                        new_text.push_str(first_char);
                        new_text.push_str(&sym_text[abs_end..]);
                        sym_text = new_text;
                    }
                }
            }
        }
    }

    s.push_str("    ");
    s.push_str(&sym_text);
    s.push('\n');
}

/// Emit all lib_symbol definitions from the library entries.
/// Fully data-driven — no hardcoded symbol names or file paths.
fn emit_lib_symbols(s: &mut String, entries: &[LibraryEntry]) {
    s.push_str("  (lib_symbols\n");
    for e in entries {
        emit_lib_symbol_from_file(s, &e.raw_content, &e.lib_prefix, &e.sym_name_in_file);
    }
    s.push_str("  )\n");
}

// ============================================================
// Step 3: KiCad output from placed components
// ============================================================

/// KiCad page margin (mm).  Positions from the DAG solver are already in mm.
const KICAD_BASE_X: f64 = 50.0;
const KICAD_BASE_Y: f64 = 50.0;

fn to_kicad_x(mm: f64) -> f64 { KICAD_BASE_X + mm }
fn to_kicad_y(mm: f64) -> f64 { KICAD_BASE_Y + mm }

/// Compute the physical KiCad (mm) connection point for a schematic node.
fn kicad_endpoint(
    node: &SchematicNode,
    matched: &[MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
) -> (f64, f64) {
    match &node.node_type {
        NodeType::Port { refdes, pin, .. } => {
            if let Some(comp) = matched.iter().find(|c| c.refdes == *refdes) {
                if !comp.pin_ki_x.is_empty() {
                    if let Some(idx) = comp.pins.iter().position(|p| p.pin_num == *pin) {
                        return (to_kicad_x(comp.pin_ki_x[idx]), to_kicad_y(comp.pin_ki_y[idx]));
                    }
                }
            }
            (to_kicad_x(col_x[node.grid_col]), to_kicad_y(row_y[node.grid_row]))
        }
        _ => (to_kicad_x(col_x[node.grid_col]), to_kicad_y(row_y[node.grid_row])),
    }
}

/// A straight wire in KiCad mm coordinates.
struct KicadWire {
    x1: f64, y1: f64, x2: f64, y2: f64,
}

/// Re-extract wires from the ASCII grid, computing endpoints in KiCad mm.
fn extract_kicad_wires(
    nodes: &[SchematicNode],
    matched: &[MatchedComponent],
    col_x: &[f64],
    row_y: &[f64],
    input: &str,
) -> Vec<KicadWire> {
    let grid: Vec<Vec<char>> = input.lines().map(|l| l.chars().collect()).collect();
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let mut node_at: std::collections::HashMap<(usize, usize), &SchematicNode> =
        std::collections::HashMap::new();
    for n in nodes {
        // Placeholder dots do not participate in wire extraction.
        if !matches!(n.node_type, NodeType::Placeholder) {
            node_at.insert((n.grid_row, n.grid_col), n);
        }
    }

    let mut wires = Vec::new();

    // ---- horizontal --------------------------------------------------------
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
                    grid.get(a.pos.row).and_then(|line| line.get(col)) == Some(&'-')
                });
            if has_dash {
                let (x1, y1) = kicad_endpoint(a, matched, col_x, row_y);
                let (x2, y2) = kicad_endpoint(b, matched, col_x, row_y);
                wires.push(KicadWire { x1, y1, x2, y2 });
            }
        }
    }

    // ---- vertical ----------------------------------------------------------
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
                    grid.get(row).and_then(|line| line.get(a.pos.col)) == Some(&'|')
                });
            if has_pipe {
                let (x1, y1) = kicad_endpoint(a, matched, col_x, row_y);
                let (x2, y2) = kicad_endpoint(b, matched, col_x, row_y);
                wires.push(KicadWire { x1, y1, x2, y2 });
            }
        }
    }

    wires
}

/// Generate KiCad S-expression with labels, wires, junctions,
/// and matched symbols using their full pin lists.
/// Uses dynamic `col_x` / `row_y` arrays from [`crate::parser::compute_layout`].
pub fn generate_step3(
    labels: &[(String, usize, usize)],
    nodes: &[SchematicNode],
    col_x: &[f64],
    row_y: &[f64],
    matched: &[MatchedComponent],
    input: &str,
    lib_entries: &[LibraryEntry],
) -> String {
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
    emit_lib_symbols(&mut s, lib_entries);

    // ---- junctions -------------------------------------------------------
    for node in nodes {
        if matches!(node.node_type, NodeType::Junction) {
            let jx = to_kicad_x(col_x[node.grid_col]);
            let jy = to_kicad_y(row_y[node.grid_row]);
            s.push_str("  (junction\n");
            s.push_str(&format!("    (at {:.2} {:.2})\n", jx, jy));
            s.push_str("    (diameter 0)\n");
            s.push_str("    (color 0 0 0 0)\n");
            s.push_str(&format!(
                "    (uuid \"{}\")\n",
                make_uuid(5000 + node.grid_row as u128 * 100 + node.grid_col as u128)
            ));
            s.push_str("  )\n");
        }
    }

    // ---- labels ----------------------------------------------------------
    for (i, (name, grid_row, grid_col)) in labels.iter().enumerate() {
        let lx = to_kicad_x(col_x[*grid_col]);
        let ly = to_kicad_y(row_y[*grid_row]);

        s.push_str("  (label\n");
        s.push_str(&format!("    \"{}\"\n", name));
        s.push_str(&format!("    (at {:.2} {:.2} 0)\n", lx, ly));
        s.push_str("    (effects\n");
        s.push_str("      (font (size 1.27 1.27))\n");
        s.push_str("      (justify left bottom)\n");
        s.push_str("    )\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(3000 + i as u128)));
        s.push_str("  )\n");
    }

    // ---- wires (native KiCad mm endpoints) -------------------------------
    let kicad_wires = extract_kicad_wires(nodes, matched, col_x, row_y, input);
    for (i, w) in kicad_wires.iter().enumerate() {
        s.push_str("  (wire\n");
        s.push_str(&format!(
            "    (pts (xy {:.2} {:.2}) (xy {:.2} {:.2}))\n",
            w.x1, w.y1, w.x2, w.y2
        ));
        s.push_str("    (stroke (width 0) (type default))\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(4000 + i as u128)));
        s.push_str("  )\n");
    }

    // ---- matched symbols --------------------------------------------------
    for (i, comp) in matched.iter().enumerate() {
        // Solver computes CW angle in canvas Y-down.
        // KiCad `at` uses CCW in Y-up, which is the inverse: 360° − CW.
        let angle_deg = (360.0 - comp.angle) % 360.0;
        let angle: i32 = angle_deg as i32;

        // Place the symbol so its anchor pin (pins[0]) lands exactly at its
        // grid position.  The anchor pin is at local KiCad offset (anchor_ki_x,
        // anchor_ki_y) from the symbol origin; subtracting that offset gives
        // the symbol origin in canvas coordinates.
        let phi = comp.angle.to_radians();
        let (sin_phi, cos_phi) = (phi.sin(), phi.cos());
        let ap_col = comp.pins[0].grid_col;
        let ap_row = comp.pins[0].grid_row;
        let ox = col_x[ap_col] - comp.anchor_ki_x * cos_phi - comp.anchor_ki_y * sin_phi;
        let oy = row_y[ap_row] - comp.anchor_ki_x * sin_phi + comp.anchor_ki_y * cos_phi;
        let (ax, ay) = (to_kicad_x(ox), to_kicad_y(oy));
        let uuid_base = 6000 + i as u128 * 100;

        // Rotate template offset (KiCad Y-up) by the KiCad CCW angle,
        // then negate Y: KiCad flips the Y component when placing
        // properties on a schematic instance.
        let rot_off = |dx: f64, dy: f64| -> (f64, f64) {
            let a = angle_deg.to_radians();
            let (s, c) = (a.sin(), a.cos());
            (dx * c - dy * s, -(dx * s + dy * c))
        };

        // Property text angle: depends on component orientation only.
        // Horizontal symbol (0/180) → horizontal text (0).
        // Vertical symbol (90/270) → vertical text (90).
        let prop_angle: i32 = if (angle - 90).abs() < 1 || (angle - 270).abs() < 1 { 90 } else { 0 };

        s.push_str("  (symbol\n");
        s.push_str(&format!(
            "    (lib_id \"{}\")\n",
            comp.lib_id
        ));
        s.push_str(&format!("    (at {:.2} {:.2} {})\n", ax, ay, angle));
        s.push_str("    (unit 1)\n");
        s.push_str("    (body_style 1)\n");
        s.push_str("    (exclude_from_sim no)\n");
        s.push_str("    (in_bom yes)\n");
        s.push_str("    (on_board yes)\n");
        s.push_str("    (in_pos_files yes)\n");
        s.push_str("    (dnp no)\n");
        s.push_str("    (fields_autoplaced yes)\n");
        s.push_str(&format!("    (uuid \"{}\")\n", make_uuid(uuid_base)));

        // Reference — uses template-defined offset rotated by component angle.
        // Power symbols (refdes starts with '#') hide the reference text.
        {
            let (rdx, rdy) = rot_off(comp.ref_ki_x, comp.ref_ki_y);
            s.push_str(&format!(
                "    (property \"Reference\" \"{}\"\n",
                comp.refdes
            ));
            s.push_str(&format!(
                "      (at {:.2} {:.2} {})\n",
                ax + rdx, ay + rdy, prop_angle
            ));
            if comp.refdes.starts_with('#') {
                s.push_str("      (hide yes)\n");
            }
            s.push_str("      (show_name no)\n");
            s.push_str("      (do_not_autoplace no)\n");
            s.push_str("      (effects (font (size 1.27 1.27)))\n");
            s.push_str("    )\n");
        }

        // Value — uses template-defined offset rotated by component angle
        {
            let (vdx, vdy) = rot_off(comp.val_ki_x, comp.val_ki_y);
            s.push_str(&format!(
                "    (property \"Value\" \"{}\"\n",
                comp.symbol_name
            ));
            s.push_str(&format!(
                "      (at {:.2} {:.2} {})\n",
                ax + vdx, ay + vdy, prop_angle
            ));
            s.push_str("      (show_name no)\n");
            s.push_str("      (do_not_autoplace no)\n");
            s.push_str("      (effects (font (size 1.27 1.27)))\n");
            s.push_str("    )\n");
        }

        // Footprint (hidden)
        {
            let (fdx, fdy) = rot_off(-2.54, 5.08);
            s.push_str("    (property \"Footprint\" \"Package_SO:SOIC-8_3.9x4.9mm_P1.27mm\"\n");
            s.push_str(&format!("      (at {:.2} {:.2} 0)\n", ax + fdx, ay + fdy));
            s.push_str("      (hide yes)\n");
            s.push_str("      (show_name no)\n");
            s.push_str("      (do_not_autoplace no)\n");
            s.push_str("      (effects (font (size 1.27 1.27)) (justify left))\n");
            s.push_str("    )\n");
        }

        // Datasheet (hidden)
        {
            let (ddx, ddy) = rot_off(3.81, -3.81);
            s.push_str("    (property \"Datasheet\" \"http://www.ti.com/lit/ds/symlink/opa330.pdf\"\n");
            s.push_str(&format!("      (at {:.2} {:.2} 0)\n", ax + ddx, ay + ddy));
            s.push_str("      (hide yes)\n");
            s.push_str("      (show_name no)\n");
            s.push_str("      (do_not_autoplace no)\n");
            s.push_str("      (effects (font (size 1.27 1.27)))\n");
            s.push_str("    )\n");
        }

        // Description (hidden)
        s.push_str("    (property \"Description\" \"50μV V OS, 0.25μV/°C, 35μA CMOS OPERATIONAL AMPLIFIERS, Zerø-Drift Series, SOIC\"\n");
        s.push_str(&format!("      (at {:.2} {:.2} 0)\n", ax, ay));
        s.push_str("      (hide yes)\n");
        s.push_str("      (show_name no)\n");
        s.push_str("      (do_not_autoplace no)\n");
        s.push_str("      (effects (font (size 1.27 1.27)))\n");
        s.push_str("    )\n");

        // Pins — ALL pin numbers (including hidden NC pins 1,5,8)
        for &pin_num in &comp.all_pin_numbers {
            let pin_uuid = make_uuid(uuid_base + pin_num as u128);
            s.push_str(&format!(
                "    (pin \"{}\" (uuid \"{}\"))\n",
                pin_num, pin_uuid
            ));
        }

        // Instances
        s.push_str("    (instances\n");
        s.push_str("      (project \"\"\n");
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
