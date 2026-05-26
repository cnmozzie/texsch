use crate::parser::{DrawPrimitive, MatchedComponent, NodeSpan, NodeType, PinDirection, SchematicNode, WireSegment, HALF_SPAN, MARGIN, CELL_W, CELL_H};

/// Convert KiCad mm to SVG px.
const SVG_PX_PER_MM: f64 = CELL_W / 2.54;

fn mm_to_px(mm: f64) -> f64 { mm * SVG_PX_PER_MM }

/// Rotate a [`PinDirection`] by `-angle_deg` so that when the SVG group is
/// rotated by `angle_deg` the pin appears to face the original direction.
fn rotate_pin_dir(dir: PinDirection, angle_deg: f64) -> PinDirection {
    if angle_deg == 0.0 { return dir; }
    let (dx, dy): (f64, f64) = match dir {
        PinDirection::Left  => (-1.0, 0.0),
        PinDirection::Right => (1.0, 0.0),
        PinDirection::Up    => (0.0, -1.0),
        PinDirection::Down  => (0.0, 1.0),
    };
    let a = (-angle_deg).to_radians(); // inverse rotation (group → local)
    let (c, s) = (a.cos(), a.sin());
    let rx = dx * c - dy * s;
    let ry = dx * s + dy * c;
    if rx.abs() > ry.abs() {
        if rx > 0.0 { PinDirection::Right } else { PinDirection::Left }
    } else {
        if ry > 0.0 { PinDirection::Down } else { PinDirection::Up }
    }
}

/// Render a debug grid showing every [`SchematicNode`] at its compressed
/// grid position, with light grey grid lines and row/column headers.
pub fn generate_grid(nodes: &[SchematicNode]) -> String {
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let width = MARGIN + (max_col as f64 + 1.0) * CELL_W + 20.0;
    let height = MARGIN + (max_row as f64 + 1.0) * CELL_H + 20.0;

    let mut elements = String::new();

    for c in 0..=max_col {
        let x = MARGIN + c as f64 * CELL_W;
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            x, MARGIN, x, MARGIN + (max_row as f64 + 1.0) * CELL_H,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#999\">C{}</text>",
            x, MARGIN - 12.0, c
        ));
    }
    for r in 0..=max_row {
        let y = MARGIN + r as f64 * CELL_H;
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            MARGIN, y, MARGIN + (max_col as f64 + 1.0) * CELL_W, y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" \
             font-size=\"12\" fill=\"#999\">R{}</text>",
            MARGIN - 12.0, y + 5.0, r
        ));
    }

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
            NodeType::Port { refdes, pin, name, dir } => {
                let name_part = if name.is_empty() { String::new() }
                    else { format!("({})", name) };
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"14\" font-weight=\"bold\" fill=\"#8B0000\">{}:{}{}{} ({},{})</text>",
                    x, y + 5.0, refdes, pin, name_part, dir.to_char(), node.grid_row, node.grid_col
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
            NodeType::Placeholder => {
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"10\" fill=\"#ccc\">. ({},{})</text>",
                    x, y + 5.0, node.grid_row, node.grid_col
                ));
            }
            NodeType::Anchor { refdes } => {
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                     font-size=\"13\" font-weight=\"bold\" fill=\"#0055AA\">{} ({},{})</text>",
                    x, y + 5.0, refdes, node.grid_row, node.grid_col
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

/// Convert a KiCad-style arc (defined by three points: start, mid, end) into
/// an SVG arc path command `A rx ry xrot large-arc sweep x y`.
fn arc_to_svg(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> String {
    let (x1, y1) = start;
    let (x2, y2) = mid;
    let (x3, y3) = end;

    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-9 {
        return format!("L {:.1} {:.1}", x3, y3);
    }

    let cx = ((x1.powi(2) + y1.powi(2)) * (y2 - y3)
        + (x2.powi(2) + y2.powi(2)) * (y3 - y1)
        + (x3.powi(2) + y3.powi(2)) * (y1 - y2))
        / d;
    let cy = ((x1.powi(2) + y1.powi(2)) * (x3 - x2)
        + (x2.powi(2) + y2.powi(2)) * (x1 - x3)
        + (x3.powi(2) + y3.powi(2)) * (x2 - x1))
        / d;
    let r = ((x1 - cx).powi(2) + (y1 - cy).powi(2)).sqrt();

    let cross = (x2 - x1) * (y3 - y1) - (y2 - y1) * (x3 - x1);
    let sweep = if cross > 0.0 { 1 } else { 0 };

    format!("A {:.1} {:.1} 0 0 {} {:.1} {:.1}", r, r, sweep, x3, y3)
}

/// Shared SVG preamble (defs + style) used by all grid renderers.
fn svg_preamble() -> String {
    let arrow_defs = r##"<defs>
    <marker id="arrow" markerWidth="6" markerHeight="6"
     refX="6" refY="3" orient="auto">
     <path d="M0,0 L6,3 L0,6 Z" fill="#8B0000"/>
    </marker>
</defs>"##;
    let style = r#"<style>
    text { font-family: monospace; }
    .span-box { fill: none; stroke-width: 1; stroke-dasharray: 4,4; }
    .span-node { stroke: #a0c0ff; }
    .span-empty { stroke: #e8e0d0; }
</style>"#;
    format!("{}{}", arrow_defs, style)
}

/// Grid spacing in px between two vertically stacked grids.
const GRID_GAP_PX: f64 = 48.0;

/// Render one grid's content elements (no outer `<svg>` wrapper).
/// `y_base` offsets all Y coordinates for stacked-grid layouts.
/// Returns `(elements_svg, width_px, height_px)`.
fn render_grid_elements(
    nodes: &[SchematicNode],
    wires: &[WireSegment],
    col_x: &[f64],
    row_y: &[f64],
    matched: &[MatchedComponent],
    y_base: f64,
) -> (String, f64, f64) {
    let max_row = nodes.iter().map(|n| n.grid_row).max().unwrap_or(0);
    let max_col = nodes.iter().map(|n| n.grid_col).max().unwrap_or(0);

    let col_px: Vec<f64> = col_x.iter().map(|&x| mm_to_px(x)).collect();
    let row_px: Vec<f64> = row_y.iter().map(|&y| mm_to_px(y) + y_base).collect();
    let margin_px = mm_to_px(MARGIN);

    let last_x = col_px[max_col];
    let last_y = row_px[max_row];
    let width = last_x + margin_px + 20.0;
    let height = last_y + margin_px + 20.0 - y_base;

    let mut elements = String::new();

    // ---- grid lines + headers --------------------------------------------
    for c in 0..=max_col {
        let x = col_px[c];
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            x, margin_px + y_base, x, last_y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#999\">C{}</text>",
            x, margin_px + y_base - 12.0, c
        ));
    }
    for r in 0..=max_row {
        let y = row_px[r];
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#e0e0e0\" stroke-width=\"1\"/>",
            margin_px, y, last_x, y,
        ));
        elements.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" \
             font-size=\"12\" fill=\"#999\">R{}</text>",
            margin_px - 12.0, y + 5.0, r
        ));
    }

    // ---- wires -----------------------------------------------------------
    for seg in wires {
        elements.push_str(&format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#1a1a1a\" stroke-width=\"2\" stroke-linecap=\"round\"/>",
            mm_to_px(seg.x1), mm_to_px(seg.y1) + y_base,
            mm_to_px(seg.x2), mm_to_px(seg.y2) + y_base,
        ));
    }

    // ---- labels ----------------------------------------------------------
    for node in nodes {
        if let NodeType::Label(name) = &node.node_type {
            let x = col_px[node.grid_col];
            let y = row_px[node.grid_row];
            elements.push_str(&format!(
                "<g data-label=\"{}\">\
                 <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                 font-size=\"14\" font-weight=\"bold\" fill=\"#1B5E20\">{}</text></g>",
                name, x, y + 5.0, name
            ));
        }
    }

    // ---- ports (red dots + text) -----------------------------------------
    for node in nodes {
        if let NodeType::Port { refdes, pin, name, dir } = &node.node_type {
            let x = col_px[node.grid_col];
            let y = row_px[node.grid_row];
            elements.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"#cc0000\"/>",
                x, y
            ));
            let name_part = if name.is_empty() { String::new() }
                else { format!("({})", name) };
            elements.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
                 font-size=\"11\" font-weight=\"bold\" fill=\"#8B0000\">{}:{}{}{} ({},{})</text>",
                x, y - 10.0, refdes, pin, name_part, dir.to_char(), node.grid_row, node.grid_col
            ));
        }
    }

    // ---- matched component symbols (unified anchor-based rendering) -------
    for comp in matched {
        let anchor_pin = &comp.pins[0];
        let ax = col_px[anchor_pin.grid_col];
        let ay = row_px[anchor_pin.grid_row];
        let angle = comp.angle;

        let off_x = anchor_pin.tmpl_phys_x * SVG_PX_PER_MM;
        let off_y = anchor_pin.tmpl_phys_y * SVG_PX_PER_MM;

        let to_px = |mm: f64| -> f64 { (mm / 2.54 * CELL_W).max(0.8) };

        if !comp.draw_primitives.is_empty() {
            elements.push_str(&format!(
                "<g transform=\"translate({:.1},{:.1}) rotate({:.0})\" data-refdes=\"{}\">",
                ax, ay, angle, comp.refdes
            ));

            for dp in &comp.draw_primitives {
                match dp {
                    DrawPrimitive::Polyline { pts, stroke_width, fill_type } => {
                        let fill = match fill_type.as_str() {
                            "background" => "rgba(255,255,180,0.25)",
                            "outline" => "#8B0000",
                            _ => "none",
                        };
                        let points_str: Vec<String> = pts.iter().map(|(gx, gy)| {
                            format!("{:.1},{:.1}",
                                gx * CELL_W - off_x,
                                gy * CELL_H - off_y)
                        }).collect();
                        let sw = to_px(*stroke_width);
                        elements.push_str(&format!(
                            "<polygon points=\"{}\" fill=\"{}\" \
                             stroke=\"#8B0000\" stroke-width=\"{:.1}\"/>",
                            points_str.join(" "), fill, sw
                        ));
                    }
                    DrawPrimitive::Rectangle { start, end, stroke_width, fill_type } => {
                        let fill = match fill_type.as_str() {
                            "background" => "rgba(255,255,180,0.25)",
                            "outline" => "#8B0000",
                            _ => "none",
                        };
                        let x = start.0.min(end.0) * CELL_W - off_x;
                        let y = start.1.min(end.1) * CELL_H - off_y;
                        let w = (end.0 - start.0).abs() * CELL_W;
                        let h = (end.1 - start.1).abs() * CELL_H;
                        let sw = to_px(*stroke_width);
                        elements.push_str(&format!(
                            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" \
                             fill=\"{}\" stroke=\"#8B0000\" stroke-width=\"{:.1}\"/>",
                            x, y, w, h, fill, sw
                        ));
                    }
                    DrawPrimitive::Arc { start, mid, end, stroke_width, fill_type } => {
                        let fill = match fill_type.as_str() {
                            "background" => "rgba(255,255,180,0.25)",
                            "outline" => "#8B0000",
                            _ => "none",
                        };
                        let sx = start.0 * CELL_W - off_x;
                        let sy = start.1 * CELL_H - off_y;
                        let mx = mid.0 * CELL_W - off_x;
                        let my = mid.1 * CELL_H - off_y;
                        let ex = end.0 * CELL_W - off_x;
                        let ey = end.1 * CELL_H - off_y;
                        let sw = to_px(*stroke_width);
                        let a = arc_to_svg((sx, sy), (mx, my), (ex, ey));
                        elements.push_str(&format!(
                            "<path d=\"M {:.1} {:.1} {}\" \
                             fill=\"{}\" stroke=\"#8B0000\" stroke-width=\"{:.1}\"/>",
                            sx, sy, a, fill, sw
                        ));
                    }
                    DrawPrimitive::Circle { center, radius, stroke_width, fill_type } => {
                        let fill = match fill_type.as_str() {
                            "background" => "rgba(255,255,180,0.25)",
                            "outline" => "#8B0000",
                            _ => "none",
                        };
                        let cx = center.0 * CELL_W - off_x;
                        let cy = center.1 * CELL_H - off_y;
                        let r = radius * CELL_W;
                        let sw = to_px(*stroke_width);
                        elements.push_str(&format!(
                            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" \
                             fill=\"{}\" stroke=\"#8B0000\" stroke-width=\"{:.1}\"/>",
                            cx, cy, r, fill, sw
                        ));
                    }
                }
            }

            // Pin connection lines
            for p in &comp.pins {
                let lx = p.tmpl_phys_x * SVG_PX_PER_MM - off_x;
                let ly = p.tmpl_phys_y * SVG_PX_PER_MM - off_y;
                let len_px = (p.pin_length_mm / 2.54) * CELL_W;
                let local_dir = p.dir;
                let (ix, iy) = match local_dir {
                    PinDirection::Left  => (lx + len_px, ly),
                    PinDirection::Right => (lx - len_px, ly),
                    PinDirection::Up    => (lx, ly + len_px),
                    PinDirection::Down  => (lx, ly - len_px),
                };
                elements.push_str(&format!(
                    "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
                     stroke=\"#8B0000\" stroke-width=\"1.2\"/>",
                    lx, ly, ix, iy
                ));

                let label = if p.name.is_empty() {
                    format!("{}:{}", comp.refdes, p.pin_num)
                } else {
                    format!("{}:{}({})", comp.refdes, p.pin_num, p.name)
                };
                elements.push_str(&format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"start\" \
                     font-size=\"9\" fill=\"#8B0000\">{}</text>",
                    lx + 8.0, ly - 8.0, label
                ));
            }

            elements.push_str("</g>");
        }

        // refdes label
        elements.push_str(&format!(
            "<g data-refdes=\"{}\">\
             <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"12\" fill=\"#8B0000\">{}</text></g>",
            comp.refdes, ax, ay - mm_to_px(3.0), comp.refdes
        ));
    }

    // ---- junction dots ---------------------------------------------------
    for node in nodes {
        if matches!(node.node_type, NodeType::Junction) {
            let cx = col_px[node.grid_col];
            let cy = row_px[node.grid_row];
            elements.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\" fill=\"#1a1a1a\"/>",
                cx, cy
            ));
        }
    }

    // ---- bounding boxes (span visualization) -----------------------------
    for r in 0..=max_row {
        for c in 0..=max_col {
            let cx = col_px[c];
            let cy = row_px[r];
            let node = nodes.iter().find(|n| n.grid_row == r && n.grid_col == c);
            let s = match node {
                Some(n) => n.span,
                None => NodeSpan { left: HALF_SPAN, right: HALF_SPAN, up: HALF_SPAN, down: HALF_SPAN },
            };
            let kind = if node.is_some() { "node" } else { "empty" };
            elements.push_str(&format!(
                "<rect class=\"span-box span-{kind}\" x=\"{x:.1}\" y=\"{y:.1}\" \
                 width=\"{w:.1}\" height=\"{h:.1}\"/>",
                kind = kind,
                x = cx - mm_to_px(s.left),
                y = cy - mm_to_px(s.up),
                w = mm_to_px(s.left + s.right),
                h = mm_to_px(s.up + s.down),
            ));
        }
    }

    (elements, width, height)
}

/// Render single-grid SVG (backward-compatible with existing callers).
/// Uses dynamic `col_x` / `row_y` arrays from [`crate::parser::compute_layout`].
pub fn generate_step3(
    nodes: &[SchematicNode],
    wires: &[WireSegment],
    col_x: &[f64],
    row_y: &[f64],
    matched: &[MatchedComponent],
) -> String {
    let (elements, width, height) =
        render_grid_elements(nodes, wires, col_x, row_y, matched, 0.0);

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" \
         viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\">{}{}</svg>",
        width, height, width, height,
        svg_preamble(),
        elements,
    )
}

/// Render dual-grid SVG: Grid 1 (main circuit) on top, dashed separator,
/// Grid 2 (preview sandbox) below.  If `grid2` is `None`, renders Grid 1 only.
pub fn generate_dual_grid(
    nodes1: &[SchematicNode],
    wires1: &[WireSegment],
    col_x1: &[f64],
    row_y1: &[f64],
    matched1: &[MatchedComponent],
    grid2: Option<(
        &[SchematicNode],
        &[WireSegment],
        &[f64],
        &[f64],
        &[MatchedComponent],
    )>,
) -> String {
    let (elements1, width1, height1) =
        render_grid_elements(nodes1, wires1, col_x1, row_y1, matched1, 0.0);

    if let Some((nodes2, wires2, col_x2, row_y2, matched2)) = grid2 {
        let (elements2, width2, height2) =
            render_grid_elements(nodes2, wires2, col_x2, row_y2, matched2, height1 + GRID_GAP_PX);

        let total_width = width1.max(width2);
        let total_height = height1 + GRID_GAP_PX + height2;

        // Dashed separator line between the two grids.
        let sep_y = height1 + GRID_GAP_PX / 2.0;
        let separator = format!(
            "<line x1=\"0\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke=\"#b0b0b0\" stroke-width=\"1.5\" stroke-dasharray=\"8,6\"/>",
            sep_y, total_width, sep_y
        );

        // Semi-transparent label for Grid 2 region.
        let label = format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" \
             font-size=\"11\" fill=\"#ccc\" font-family=\"monospace\">Component Preview</text>",
            total_width / 2.0, height1 + GRID_GAP_PX - 6.0,
        );

        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\">{}{}{}{}{}</svg>",
            total_width, total_height, total_width, total_height,
            svg_preamble(),
            elements1,
            separator,
            label,
            elements2,
        )
    } else {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"0 0 {:.0} {:.0}\" width=\"{:.0}\" height=\"{:.0}\">{}{}</svg>",
            width1, height1, width1, height1,
            svg_preamble(),
            elements1,
        )
    }
}
