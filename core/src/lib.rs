pub mod kicad;
pub mod parser;
pub mod svg;
use wasm_bindgen::prelude::*;

/// Component type — MVP covers R, L, C only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompType {
    Resistor,
    Capacitor,
    Inductor,
}

impl CompType {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'R' => Some(Self::Resistor),
            'C' => Some(Self::Capacitor),
            'L' => Some(Self::Inductor),
            _ => None,
        }
    }

    pub fn lib_name(&self) -> &'static str {
        match self {
            Self::Resistor => "Device:R",
            Self::Capacitor => "Device:C",
            Self::Inductor => "Device:L",
        }
    }

    pub fn letter(&self) -> char {
        match self {
            Self::Resistor => 'R',
            Self::Capacitor => 'C',
            Self::Inductor => 'L',
        }
    }

    /// Visual half-width of the symbol in layout units (px for SVG).
    pub fn symbol_half_width(&self) -> f64 {
        match self {
            Self::Resistor => 30.0,
            Self::Capacitor => 28.0,
            Self::Inductor => 30.0,
        }
    }
}

/// Estimated pixel width of one monospace character at the SVG font size.
/// Tune this to match the actual rendered font metrics.
pub const CHAR_WIDTH: f64 = 8.0;

/// Estimated pixel height of a line of text at the SVG font size.
pub const LABEL_TEXT_H: f64 = 12.0;

/// A placed component extracted from the schematic source.
#[derive(Debug, Clone)]
pub struct Component {
    pub refdes: String,
    pub comp_type: CompType,
    pub value: String,
    /// Horizontal position in layout units.
    pub x: f64,
    /// Vertical position (fixed for single-line MVP).
    pub y: f64,
}

/// One endpoint of a net connection.
#[derive(Debug, Clone)]
pub enum NetEndpoint {
    Label(String),
    ComponentPin { refdes: String, pin: usize },
}

/// A wire connecting two net endpoints.
#[derive(Debug, Clone)]
pub struct NetSegment {
    pub from: NetEndpoint,
    pub to: NetEndpoint,
}

/// The complete parsed circuit graph.
#[derive(Debug, Clone)]
pub struct Circuit {
    pub components: Vec<Component>,
    pub connections: Vec<NetSegment>,
    /// Label → x-position map for rendering (populated by 2D pipeline).
    pub label_x: Vec<(String, f64)>,
}

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
/// Step 1: scan, Step 2: compress, Step 3: pair & render.
#[wasm_bindgen]
pub fn compile(input: &str) -> CompileResult {
    let mut nodes = parser::scan_nodes(input);
    parser::compress_coordinates(&mut nodes);

    let (placed, errors) = parser::pair_components(&nodes);
    parser::compute_spans(&mut nodes, &placed);
    let (col_x, row_y) = parser::compute_layout(&nodes);

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

    let debug = format_debug(&nodes, &placed, &errors);
    let wires = parser::extract_wires(&nodes, &placed, &col_x, &row_y, input);
    let svg = svg::generate_step3(&nodes, &placed, &wires, &col_x, &row_y);
    let kicad_sch = kicad::generate_step3(&placed, &labels, &nodes, &col_x, &row_y, input);

    CompileResult { svg, kicad_sch, debug }
}

fn format_debug(
    nodes: &[parser::SchematicNode],
    placed: &[parser::PlacedComponent],
    errors: &[String],
) -> String {
    use parser::{NodeType, Orientation};
    let mut s = format!("Step 3 — {} nodes, {} components:\n", nodes.len(), placed.len());

    for node in nodes {
        let kind = match &node.node_type {
            NodeType::Port { refdes, pin } => format!("Port({}:{})", refdes, pin),
            NodeType::Label(name) => format!("Label([{}])", name),
            NodeType::Junction => "Junction(*)".to_string(),
            NodeType::Corner => "Corner(+)".to_string(),
        };
        s.push_str(&format!(
            "  abs=({}, {})  grid=(R{}, C{})  {}  width={}\n",
            node.pos.row, node.pos.col, node.grid_row, node.grid_col, kind, node.text_width
        ));
    }

    if !placed.is_empty() {
        s.push_str("Placed:\n");
        for comp in placed {
            let ori = match comp.orientation {
                Orientation::Horizontal => "H",
                Orientation::Vertical => "V",
            };
            s.push_str(&format!(
                "  {}  type={:?}  ori={}  center=(R{:.1}, C{:.1})\n",
                comp.refdes, comp.comp_type, ori, comp.center_row, comp.center_col
            ));
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

    #[test]
    fn step3_valid_horizontal_and_vertical() {
        let result = compile("[VCC]   R1:1   R1:2\n        C1:1\n        C1:2\n");
        assert!(result.debug.contains("Step 3"));
        assert!(result.debug.contains("Placed:"));
        assert!(result.debug.contains("ori=H"));
        assert!(result.debug.contains("ori=V"));
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("C0"));
        // No errors expected
        assert!(!result.debug.contains("Errors:"));
        // KiCad has symbols
        assert!(result.kicad_sch.contains("Device:R"));
        assert!(result.kicad_sch.contains("Device:C"));
        assert!(result.kicad_sch.contains("label"));
    }

    #[test]
    fn step3_non_adjacent_pins_reported() {
        let result = compile("R2:1    +    R2:2\n");
        assert!(result.debug.contains("Step 3"));
        assert!(result.debug.contains("Errors:"));
        assert!(result.debug.contains("not adjacent"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("kicad_sch"));
    }

    #[test]
    fn step3_empty_input() {
        let result = compile("");
        assert!(result.debug.contains("0 nodes"));
        assert!(result.debug.contains("0 components"));
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
        let result = compile("L1:1  L1:2\n");
        assert!(result.debug.contains("Placed:"));
        assert!(result.debug.contains("ori=H"));
        assert!(result.debug.contains("Inductor"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:L"));
    }

    #[test]
    fn step3_vertical_resistor() {
        let result = compile("R3:1\nR3:2\n");
        assert!(result.debug.contains("Placed:"));
        assert!(result.debug.contains("ori=V"));
        assert!(result.debug.contains("Resistor"));
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("Device:R"));
    }
}
