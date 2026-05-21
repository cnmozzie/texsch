use crate::{Circuit, CompType, NetEndpoint};

const COMP_SPAN: f64 = 60.0;
const Y_CENTER: f64 = 100.0;
const FONT_SIZE: f64 = 12.0;

const COLOR_BLACK: &str = "#000";
const COLOR_GRAY: &str = "#555";
const COLOR_DARK_GREEN: &str = "#005";

pub fn generate(circuit: &Circuit) -> String {
    let mut elements = String::new();
    let mut max_x: f64 = 20.0;

    // Draw every connection as a horizontal wire between endpoint positions.
    for seg in &circuit.connections {
        let x1 = endpoint_x(&seg.from, circuit);
        let x2 = endpoint_x(&seg.to, circuit);
        let xl = x1.min(x2);
        let xr = x1.max(x2);
        elements.push_str(&format!(
            r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
            xl, Y_CENTER, xr, Y_CENTER, COLOR_BLACK
        ));
        if xr > max_x {
            max_x = xr;
        }
    }

    // Draw each component symbol.
    for comp in &circuit.components {
        match comp.comp_type {
            CompType::Resistor => draw_resistor(&mut elements, comp.x, comp.y),
            CompType::Capacitor => draw_capacitor(&mut elements, comp.x, comp.y),
            CompType::Inductor => draw_inductor(&mut elements, comp.x, comp.y),
        }
        // Refdes label above the symbol
        elements.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" fill="{}">{}</text>"#,
            comp.x,
            comp.y - 16.0,
            FONT_SIZE,
            COLOR_BLACK,
            comp.refdes
        ));
        // Value label below the symbol
        elements.push_str(&format!(
            r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" fill="{}">{}</text>"#,
            comp.x,
            comp.y + 24.0,
            FONT_SIZE,
            COLOR_GRAY,
            comp.value
        ));
        if comp.x + comp.comp_type.symbol_half_width() > max_x {
            max_x = comp.x + comp.comp_type.symbol_half_width();
        }
    }

    // Draw label text for every net endpoint that is a Label.
    let mut drawn_labels: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for seg in &circuit.connections {
        for ep in [&seg.from, &seg.to] {
            if let NetEndpoint::Label(name) = ep {
                if drawn_labels.contains(name) {
                    continue;
                }
                drawn_labels.insert(name.clone());
                let x = endpoint_x(ep, circuit);
                elements.push_str(&format!(
                    r#"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="{:.0}" font-weight="bold" fill="{}">{}"#,
                    x,
                    Y_CENTER - 14.0,
                    FONT_SIZE,
                    COLOR_DARK_GREEN,
                    name
                ));
                elements.push_str("</text>");
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

/// Compute the x-coordinate for a given net endpoint.
fn endpoint_x(ep: &NetEndpoint, circuit: &Circuit) -> f64 {
    match ep {
        NetEndpoint::Label(_) => estimate_label_x(ep, circuit),
        NetEndpoint::ComponentPin { refdes, pin } => {
            let comp = circuit
                .components
                .iter()
                .find(|c| c.refdes == *refdes)
                .unwrap();
            let hw = comp.comp_type.symbol_half_width();
            if *pin == 0 {
                comp.x - hw
            } else {
                comp.x + hw
            }
        }
    }
}

fn estimate_label_x(ep: &NetEndpoint, circuit: &Circuit) -> f64 {
    let target = match ep {
        NetEndpoint::Label(name) => name.as_str(),
        _ => return 20.0,
    };

    for seg in &circuit.connections {
        if let NetEndpoint::Label(name) = &seg.from {
            if name == target {
                return 20.0;
            }
        }
        if let NetEndpoint::Label(name) = &seg.to {
            if name == target {
                let rx = endpoint_x(&seg.from, circuit);
                return rx + 50.0;
            }
        }
    }

    20.0
}

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
        d, COLOR_BLACK
    ));
}

fn draw_capacitor(buf: &mut String, cx: f64, cy: f64) {
    let gap = 6.0;
    let h = 18.0;
    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx - gap, cy - h,
        cx - gap, cy + h,
        COLOR_BLACK,
    ));
    buf.push_str(&format!(
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5"/>"#,
        cx + gap, cy - h,
        cx + gap, cy + h,
        COLOR_BLACK,
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
        d, COLOR_BLACK
    ));
}
