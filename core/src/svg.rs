use crate::{Circuit, CompType, NetEndpoint};

const COMP_SPAN: f64 = 60.0;
const Y_CENTER: f64 = 100.0;
const FONT_SIZE: f64 = 12.0;

const COLOR_COMPONENT: &str = "#8B0000";
const COLOR_WIRE: &str = "#2E7D32";
const COLOR_NETLABEL: &str = "#1B5E20";

/// SVG pixels per grid column (2D pipeline only).
const SVG_X_SCALE: f64 = 12.0;
const SVG_X_MARGIN: f64 = 20.0;

fn grid_to_svg(grid_col: f64) -> f64 {
    SVG_X_MARGIN + grid_col * SVG_X_SCALE
}

/// Symbol half-width in SVG px for a component type.
fn symbol_hw(ct: CompType) -> f64 {
    match ct {
        CompType::Resistor => 30.0,
        CompType::Capacitor => 6.0,
        CompType::Inductor => 30.0,
    }
}

pub fn generate(circuit: &Circuit) -> String {
    let is_2d = !circuit.label_x.is_empty();

    // --- pre-compute SVG-pixel x for every component --------------------
    let comp_x: Vec<f64> = circuit
        .components
        .iter()
        .map(|c| if is_2d { grid_to_svg(c.x) } else { c.x })
        .collect();

    // --- pre-compute SVG-pixel x for every label -----------------------
    let label_x: Vec<(String, f64)> = if is_2d {
        circuit
            .label_x
            .iter()
            .map(|(n, x)| (n.clone(), grid_to_svg(*x)))
            .collect()
    } else {
        // Legacy pipeline: infer label positions from connections.
        let mut out = Vec::new();
        for seg in &circuit.connections {
            for (ep, side) in [(&seg.from, 0usize), (&seg.to, 1)] {
                if let NetEndpoint::Label(name) = ep {
                    if out.iter().any(|(n, _)| n == name) {
                        continue;
                    }
                    let lx = if side == 0 {
                        20.0 // left-side labels at margin
                    } else {
                        // right-side label: after the from endpoint
                        let from_x = endpoint_x(&seg.from, &comp_x, &circuit.components, &out);
                        from_x + 50.0
                    };
                    out.push((name.clone(), lx));
                }
            }
        }
        out
    };

    let mut elements = String::new();
    let mut max_x: f64 = 20.0;

    // ---- wires --------------------------------------------------------
    for seg in &circuit.connections {
        let x1 = endpoint_x(&seg.from, &comp_x, &circuit.components, &label_x);
        let x2 = endpoint_x(&seg.to, &comp_x, &circuit.components, &label_x);
        let xl = x1.min(x2);
        let xr = x1.max(x2);
        elements.push_str(&format!(
            r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
            xl, Y_CENTER, xr, Y_CENTER, COLOR_WIRE
        ));
        if xr > max_x {
            max_x = xr;
        }
    }

    // ---- component symbols --------------------------------------------
    for (i, comp) in circuit.components.iter().enumerate() {
        let cx = comp_x[i];
        match comp.comp_type {
            CompType::Resistor => draw_resistor(&mut elements, cx, Y_CENTER),
            CompType::Capacitor => draw_capacitor(&mut elements, cx, Y_CENTER),
            CompType::Inductor => draw_inductor(&mut elements, cx, Y_CENTER),
        }
        elements.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" fill="{}">{}</text>"#,
            cx, Y_CENTER - 16.0, FONT_SIZE, COLOR_COMPONENT, comp.refdes
        ));
        elements.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" fill="{}">{}</text>"#,
            cx, Y_CENTER + 24.0, FONT_SIZE, COLOR_COMPONENT, comp.value
        ));
        if cx + symbol_hw(comp.comp_type) > max_x {
            max_x = cx + symbol_hw(comp.comp_type);
        }
    }

    // ---- net labels ---------------------------------------------------
    let mut drawn: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for seg in &circuit.connections {
        for ep in [&seg.from, &seg.to] {
            if let NetEndpoint::Label(name) = ep {
                if drawn.contains(name) {
                    continue;
                }
                drawn.insert(name.clone());
                let lx = label_x.iter().find(|(n, _)| n == name).map(|(_, x)| *x).unwrap_or(20.0);
                elements.push_str(&format!(
                    r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" font-weight="bold" fill="{}">{}"#,
                    lx, Y_CENTER - 14.0, FONT_SIZE, COLOR_NETLABEL, name
                ));
                elements.push_str("</text>");
            }
        }
    }
    // Orphan labels (from label_x but not in any connection).
    for (name, lx) in &label_x {
        if !drawn.contains(name) {
            drawn.insert(name.clone());
            elements.push_str(&format!(
                r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" font-weight="bold" fill="{}">{}"#,
                lx, Y_CENTER - 14.0, FONT_SIZE, COLOR_NETLABEL, name
            ));
            elements.push_str("</text>");
            if *lx > max_x {
                max_x = *lx;
            }
        }
    }

    let width = (max_x + 30.0).max(200.0);
    let height: f64 = 200.0;

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {:.0} {:.0}" width="{:.0}" height="{:.0}">{}</svg>"#,
        width, height, width, height, elements
    )
}

// -----------------------------------------------------------------------
// position helpers
// -----------------------------------------------------------------------

fn endpoint_x(
    ep: &NetEndpoint,
    comp_x: &[f64],
    comps: &[crate::Component],
    label_x: &[(String, f64)],
) -> f64 {
    match ep {
        NetEndpoint::Label(name) => {
            label_x.iter().find(|(n, _)| n == name).map(|(_, x)| *x).unwrap_or(20.0)
        }
        NetEndpoint::ComponentPin { refdes, pin } => {
            let i = comps.iter().position(|c| c.refdes == *refdes).unwrap();
            let cx = comp_x[i];
            let hw = symbol_hw(comps[i].comp_type);
            if *pin == 0 { cx - hw } else { cx + hw }
        }
    }
}

// -----------------------------------------------------------------------
// symbol drawing
// -----------------------------------------------------------------------

fn draw_resistor(buf: &mut String, cx: f64, cy: f64) {
    let half = COMP_SPAN / 2.0;
    let h = 14.0;
    let segs = 6;
    let step = COMP_SPAN / segs as f64;
    let mut d = format!("M {:.1} {:.1} ", cx - half, cy);
    for i in 0..segs {
        let sx = cx - half + step * i as f64 + step / 2.0;
        let sy = if i % 2 == 0 { cy - h } else { cy + h };
        d.push_str(&format!("L {:.1} {:.1} ", sx, sy));
    }
    d.push_str(&format!("L {:.1} {:.1}", cx + half, cy));
    buf.push_str(&format!(
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
        d, COLOR_COMPONENT
    ));
}

fn draw_capacitor(buf: &mut String, cx: f64, cy: f64) {
    let half = COMP_SPAN / 2.0;
    let gap = 6.0;
    let h = 18.0;

    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx - half, cy, cx - gap, cy, COLOR_COMPONENT,
    ));
    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx - gap, cy - h, cx - gap, cy + h, COLOR_COMPONENT,
    ));
    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx + gap, cy - h, cx + gap, cy + h, COLOR_COMPONENT,
    ));
    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx + gap, cy, cx + half, cy, COLOR_COMPONENT,
    ));
}

fn draw_inductor(buf: &mut String, cx: f64, cy: f64) {
    let half = COMP_SPAN / 2.0;
    let r = 8.0;
    let loops = 4;
    let spacing = COMP_SPAN / (loops as f64 + 1.0);
    let mut d = format!("M {:.1} {:.1} ", cx - half, cy);
    for i in 0..loops {
        let sx = cx - half + spacing * i as f64 + spacing;
        let sweep = if i % 2 == 0 { 1 } else { 0 };
        d.push_str(&format!(
            "A {r:.1} {r:.1} 0 0 {} {:.1} {:.1} ",
            sweep, sx, cy
        ));
    }
    d.push_str(&format!("L {:.1} {:.1}", cx + half, cy));
    buf.push_str(&format!(
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
        d, COLOR_COMPONENT
    ));
}

// ============================================================
// Step 2: Grid Visualization Rendering
// ============================================================

use crate::parser::{NodeSpan, NodeType, SchematicNode, WireSegment, HALF_SPAN, MARGIN};

pub const CELL_W: f64 = 60.0;
pub const CELL_H: f64 = 60.0;

/// Render a debug grid showing every [`SchematicNode`] at its compressed
/// grid position `(grid_col * CELL_W, grid_row * CELL_H)`, with light grey
/// grid lines and row/column headers.
pub fn generate_grid(nodes: &[SchematicNode]) -> String {
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let width = MARGIN + (max_col as f64 + 1.0) * CELL_W + 20.0;
    let height = MARGIN + (max_row as f64 + 1.0) * CELL_H + 20.0;

    let mut elements = String::new();

    // ---- vertical grid lines + column headers --------------------------
    for c in 0..=max_col {
        let x = MARGIN + c as f64 * CELL_W;
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            x,
            MARGIN,
            x,
            MARGIN + (max_row as f64 + 1.0) * CELL_H,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#999\">C{}</text>",
            x,
            MARGIN - 12.0,
            c
        ));
    }

    // ---- horizontal grid lines + row headers ---------------------------
    for r in 0..=max_row {
        let y = MARGIN + r as f64 * CELL_H;
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            MARGIN,
            y,
            MARGIN + (max_col as f64 + 1.0) * CELL_W,
            y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" \
             font-size=\"12\" fill=\"#999\">R{}</text>",
            MARGIN - 12.0,
            y + 5.0,
            r
        ));
    }

    // ---- nodes ---------------------------------------------------------
    for node in nodes {
        let x = MARGIN + node.grid_col as f64 * CELL_W;
        let y = MARGIN + node.grid_row as f64 * CELL_H;

        match &node.node_type {
            NodeType::Label(name) => {
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"14\" font-weight=\"bold\" fill=\"#1B5E20\">[{}] ({},{})</text>",
                    x, y + 5.0, name, node.grid_row, node.grid_col
                ));
            }
            NodeType::Port { refdes, pin } => {
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"14\" font-weight=\"bold\" fill=\"#8B0000\">{}:{} ({},{})</text>",
                    x, y + 5.0, refdes, pin, node.grid_row, node.grid_col
                ));
            }
            NodeType::Junction => {
                elements.push_str(&format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"#2E7D32\"/>",
                    x, y
                ));
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"12\" fill=\"#2E7D32\">* ({},{})</text>",
                    x, y - 14.0, node.grid_row, node.grid_col
                ));
            }
            NodeType::Corner => {
                elements.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"10\" height=\"10\" \
                     fill=\"none\" stroke=\"#666\" stroke-width=\"1.5\"/>",
                    x - 5.0, y - 5.0
                ));
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"12\" fill=\"#666\">+ ({},{})</text>",
                    x, y - 14.0, node.grid_row, node.grid_col
                ));
            }
        }
    }

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\">{}</svg>",
        width, height, width, height, elements
    )
}

// ============================================================
// Step 3: Component Symbol Rendering
// ============================================================

use crate::parser::{Orientation, PlacedComponent};

const SYMBOL_SPAN: f64 = 56.0; // total symbol width in SVG px

/// Render grid + labels + placed component symbols + wires + junction dots.
/// Uses dynamic `col_x` / `row_y` arrays from [`crate::parser::compute_layout`].
pub fn generate_step3(
    nodes: &[SchematicNode],
    placed: &[PlacedComponent],
    wires: &[WireSegment],
    col_x: &[f64],
    row_y: &[f64],
) -> String {
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let last_x = col_x[max_col];
    let last_y = row_y[max_row];
    let width = last_x + MARGIN + 20.0;
    let height = last_y + MARGIN + 20.0;

    let mut elements = String::new();

    // ---- grid lines + headers (dynamic positions) -----------------------
    for c in 0..=max_col {
        let x = col_x[c];
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            x, MARGIN, x, last_y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#999\">C{}</text>",
            x, MARGIN - 12.0, c
        ));
    }
    for r in 0..=max_row {
        let y = row_y[r];
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            MARGIN, y, last_x, y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" \
             font-size=\"12\" fill=\"#999\">R{}</text>",
            MARGIN - 12.0, y + 5.0, r
        ));
    }

    // ---- wires -----------------------------------------------------------
    for seg in wires {
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#1a1a1a\" stroke-width=\"2\" stroke-linecap=\"round\"/>",
            seg.x1, seg.y1, seg.x2, seg.y2
        ));
    }

    // ---- labels ----------------------------------------------------------
    for node in nodes {
        if let NodeType::Label(name) = &node.node_type {
            let x = col_x[node.grid_col];
            let y = row_y[node.grid_row];
            elements.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                 font-size=\"14\" font-weight=\"bold\" fill=\"#1B5E20\">{}</text>",
                x, y + 5.0, name
            ));
        }
    }

    // ---- component symbols -----------------------------------------------
    for comp in placed {
        let (cx, cy) = crate::parser::component_physical_center(comp, col_x, row_y);

        let rot = match comp.orientation {
            Orientation::Horizontal => 0,
            Orientation::Vertical => 90,
        };

        elements.push_str(&format!(
            "<g transform=\"rotate({},{:.1},{:.1})\">", rot, cx, cy
        ));

        match comp.comp_type {
            crate::CompType::Resistor => draw_resistor_at(&mut elements, cx, cy),
            crate::CompType::Capacitor => draw_capacitor_at(&mut elements, cx, cy),
            crate::CompType::Inductor => draw_inductor_at(&mut elements, cx, cy),
        }

        // refdes label above symbol
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#8B0000\">{}</text>",
            cx, cy - 20.0, comp.refdes
        ));

        elements.push_str("</g>");
    }

    // ---- junction dots ---------------------------------------------------
    for node in nodes {
        if matches!(node.node_type, NodeType::Junction) {
            let cx = col_x[node.grid_col];
            let cy = row_y[node.grid_row];
            elements.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\" fill=\"#1a1a1a\"/>",
                cx, cy
            ));
        }
        // Corner nodes ('+') get NO dot — they are pure wire crossings
    }

    // ---- bounding boxes (span visualization) ---------------------------
    for r in 0..=max_row {
        for c in 0..=max_col {
            let cx = col_x[c];
            let cy = row_y[r];

            let node = nodes.iter().find(|n| n.grid_row == r && n.grid_col == c);
            let s = match node {
                Some(n) => n.span,
                None => NodeSpan {
                    left: HALF_SPAN,
                    right: HALF_SPAN,
                    up: HALF_SPAN,
                    down: HALF_SPAN,
                },
            };

            let kind = if node.is_some() { "node" } else { "empty" };
            elements.push_str(&format!(
                "<rect class=\"span-box span-{kind}\" x=\"{x:.1}\" y=\"{y:.1}\" \
                 width=\"{w:.1}\" height=\"{h:.1}\"/>",
                kind = kind,
                x = cx - s.left,
                y = cy - s.up,
                w = s.left + s.right,
                h = s.up + s.down,
            ));
        }
    }

    let style = r#"<style>
    text { font-family: monospace; }
    .span-box { fill: none; stroke-width: 1; stroke-dasharray: 4,4; }
    .span-node { stroke: #a0c0ff; }
    .span-empty { stroke: #e8e0d0; }
</style>"#;

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\">{}{}</svg>",
        width, height, width, height, style, elements
    )
}

// ---- Step 3 symbol drawing helpers (origin-centred, horizontal) ---------

fn draw_resistor_at(buf: &mut String, cx: f64, cy: f64) {
    let half = SYMBOL_SPAN / 2.0;
    let h = 12.0;
    let segs = 6;
    let step = SYMBOL_SPAN / segs as f64;
    let mut d = format!("M {:.1} {:.1} ", cx - half, cy);
    for i in 0..segs {
        let sx = cx - half + step * i as f64 + step / 2.0;
        let sy = if i % 2 == 0 { cy - h } else { cy + h };
        d.push_str(&format!("L {:.1} {:.1} ", sx, sy));
    }
    d.push_str(&format!("L {:.1} {:.1}", cx + half, cy));
    buf.push_str(&format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        d
    ));
}

fn draw_capacitor_at(buf: &mut String, cx: f64, cy: f64) {
    let half = SYMBOL_SPAN / 2.0;
    let gap = 5.0;
    let h = 16.0;

    buf.push_str(&format!(
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
         stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        cx - half, cy, cx - gap, cy,
    ));
    buf.push_str(&format!(
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
         stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        cx - gap, cy - h, cx - gap, cy + h,
    ));
    buf.push_str(&format!(
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
         stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        cx + gap, cy - h, cx + gap, cy + h,
    ));
    buf.push_str(&format!(
        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
         stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        cx + gap, cy, cx + half, cy,
    ));
}

fn draw_inductor_at(buf: &mut String, cx: f64, cy: f64) {
    let half = SYMBOL_SPAN / 2.0;
    let r = 7.0;
    let loops = 4;
    let spacing = SYMBOL_SPAN / (loops as f64 + 1.0);
    let mut d = format!("M {:.1} {:.1} ", cx - half, cy);
    for i in 0..loops {
        let sx = cx - half + spacing * i as f64 + spacing;
        let sweep = if i % 2 == 0 { 1 } else { 0 };
        d.push_str(&format!(
            "A {r:.1} {r:.1} 0 0 {} {:.1} {:.1} ",
            sweep, sx, cy
        ));
    }
    d.push_str(&format!("L {:.1} {:.1}", cx + half, cy));
    buf.push_str(&format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"#8B0000\" stroke-width=\"1.5\"/>",
        d
    ));
}
