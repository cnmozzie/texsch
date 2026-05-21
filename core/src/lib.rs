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
            Self::Capacitor => 6.0,
            Self::Inductor => 30.0,
        }
    }
}

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
}

/// Output of the `compile` pipeline.
#[wasm_bindgen]
#[derive(Debug)]
pub struct CompileResult {
    #[wasm_bindgen(getter_with_clone)]
    pub svg: String,
    #[wasm_bindgen(getter_with_clone)]
    pub kicad_sch: String,
}

/// Parse the ASCII schematic `input` and produce SVG + KiCad S-expression output.
#[wasm_bindgen]
pub fn compile(input: &str) -> CompileResult {
    let tokens = parser::tokenize(input);
    let circuit = parser::build_circuit(&tokens);
    let svg = svg::generate(&circuit);
    let kicad_sch = kicad::generate(&circuit);
    CompileResult { svg, kicad_sch }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn full_pipeline_rlc() {
        let result = compile("GND --- -R1(24.9)- --- -C1(10u)- --- VCC");
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("R1"));
        assert!(result.svg.contains("C1"));
        assert!(result.kicad_sch.contains("kicad_sch"));
        assert!(result.kicad_sch.contains("Device:R"));
        assert!(result.kicad_sch.contains("Device:C"));
    }

    #[test]
    fn full_pipeline_lc() {
        let result = compile("-L1(10mH)- --- -C2(47u)- --- GND");
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("L1"));
        assert!(result.svg.contains("C2"));
        assert!(result.kicad_sch.contains("Device:L"));
        assert!(result.kicad_sch.contains("Device:C"));
    }

    #[test]
    fn empty_input() {
        let result = compile("");
        assert!(result.svg.contains("<svg"));
        assert!(result.kicad_sch.contains("kicad_sch"));
    }

    #[test]
    fn single_component() {
        let result = compile("-R1(100)-");
        assert!(result.svg.contains("R1"));
        assert!(result.svg.contains("100"));
    }
}
