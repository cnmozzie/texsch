use crate::{Circuit, CompType, Component, NetEndpoint, NetSegment};

/// Token produced by the lexer from an ASCII schematic word.
#[derive(Debug, Clone)]
pub enum Token {
    Label(String),
    Wire,
    Component {
        refdes: String,
        comp_type: CompType,
        value: String,
    },
}

/// Split the input line into whitespace-delimited tokens and classify each one.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();

    for word in input.split_whitespace() {
        if word.is_empty() {
            continue;
        }

        // All-dash token → wire
        if word.chars().all(|c| c == '-') {
            tokens.push(Token::Wire);
            continue;
        }

        // Component pattern: -<REFDES>(<VALUE>)-  e.g. -R1(10k)-
        if let Some(tok) = try_parse_component(word) {
            tokens.push(tok);
            continue;
        }

        // Everything else is a net label
        tokens.push(Token::Label(word.to_string()));
    }

    tokens
}

/// Try to match `-R1(10k)-` style component tokens.
fn try_parse_component(word: &str) -> Option<Token> {
    if !word.starts_with('-') || !word.ends_with('-') {
        return None;
    }
    let inner = &word[1..word.len() - 1];
    let paren_open = inner.find('(')?;
    let paren_close = inner.rfind(')')?;
    if paren_open >= paren_close {
        return None;
    }

    let refdes = &inner[..paren_open];
    let value = &inner[paren_open + 1..paren_close];

    let type_char = refdes.chars().next()?;
    let comp_type = CompType::from_char(type_char)?;

    // refdes must have at least one digit after the type letter
    if refdes.len() < 2 || !refdes[1..].chars().any(|c| c.is_ascii_digit()) {
        return None;
    }

    Some(Token::Component {
        refdes: refdes.to_string(),
        comp_type,
        value: value.to_string(),
    })
}

/// Walk the linear token stream and produce a `Circuit` with placed components
/// and connections.
///
/// The stream alternates between labels/wires and components.  Wires are
/// transparent in the netlist (they preserve continuity), while labels and
/// component pins become net endpoints.
pub fn build_circuit(tokens: &[Token]) -> Circuit {
    let mut components: Vec<Component> = Vec::new();
    let mut connections: Vec<NetSegment> = Vec::new();

    // Layout constants
    const Y: f64 = 100.0;
    const LABEL_W: f64 = 50.0;
    const WIRE_W: f64 = 40.0;
    const COMP_W: f64 = 80.0;
    const COMP_PAD: f64 = 10.0; // padding around component symbol

    let mut x: f64 = 20.0; // left margin
    let mut prev_endpoint: Option<NetEndpoint> = None;

    // Helper: record a component and connect it to the previous endpoint.
    let emit_component =
        |components: &mut Vec<Component>,
         connections: &mut Vec<NetSegment>,
         prev: &mut Option<NetEndpoint>,
         x: &mut f64,
         refdes: &str,
         comp_type: CompType,
         value: &str| {
            let comp = Component {
                refdes: refdes.to_string(),
                comp_type,
                value: value.to_string(),
                x: *x + COMP_PAD,
                y: Y,
            };
            components.push(comp);

            let pin0 = NetEndpoint::ComponentPin {
                refdes: refdes.to_string(),
                pin: 0,
            };
            let pin1 = NetEndpoint::ComponentPin {
                refdes: refdes.to_string(),
                pin: 1,
            };

            if let Some(ep) = prev.take() {
                connections.push(NetSegment {
                    from: ep,
                    to: pin0.clone(),
                });
            }
            *prev = Some(pin1);
            *x += COMP_PAD + COMP_W + COMP_PAD;
        };

    for tok in tokens {
        match tok {
            Token::Label(name) => {
                let label_ep = NetEndpoint::Label(name.clone());
                if let Some(ep) = prev_endpoint.take() {
                    connections.push(NetSegment {
                        from: ep,
                        to: label_ep,
                    });
                } else {
                    prev_endpoint = Some(label_ep);
                }
                x += LABEL_W;
            }
            Token::Wire => {
                x += WIRE_W;
            }
            Token::Component {
                refdes,
                comp_type,
                value,
            } => {
                emit_component(
                    &mut components,
                    &mut connections,
                    &mut prev_endpoint,
                    &mut x,
                    refdes,
                    *comp_type,
                    value,
                );
            }
        }
    }

    // If the stream ends with a dangling endpoint (unlikely), drop it —
    // an unconnected pin is not an error at the MVP level.

    Circuit {
        components,
        connections,
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn tokenize_simple_rlc_chain() {
        let tokens = tokenize("GND --- -R1(24.9)- --- -C1(10u)- --- VCC");
        let kinds: Vec<String> = tokens
            .iter()
            .map(|t| match t {
                Token::Label(s) => format!("Label({})", s),
                Token::Wire => "Wire".into(),
                Token::Component { refdes, value, .. } => {
                    format!("Comp({}={})", refdes, value)
                }
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "Label(GND)",
                "Wire",
                "Comp(R1=24.9)",
                "Wire",
                "Comp(C1=10u)",
                "Wire",
                "Label(VCC)"
            ]
        );
    }

    #[test]
    fn tokenize_inductor() {
        let tokens = tokenize("-L3(4.7mH)-");
        assert_eq!(tokens.len(), 1);
        match &tokens[0] {
            Token::Component {
                refdes,
                comp_type,
                value,
            } => {
                assert_eq!(refdes, "L3");
                assert_eq!(*comp_type, CompType::Inductor);
                assert_eq!(value, "4.7mH");
            }
            _ => panic!("expected component token"),
        }
    }

    #[test]
    fn tokenize_invalid_component_becomes_label() {
        // Missing closing dash
        let tokens = tokenize("-X1(5)");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], Token::Label(_)));
    }

    #[test]
    fn build_circuit_rlc_chain() {
        let tokens = tokenize("GND --- -R1(10k)- --- -C1(10u)- --- VCC");
        let circuit = build_circuit(&tokens);

        assert_eq!(circuit.components.len(), 2);
        assert_eq!(circuit.components[0].refdes, "R1");
        assert_eq!(circuit.components[0].value, "10k");
        assert_eq!(circuit.components[1].refdes, "C1");
        assert_eq!(circuit.components[1].value, "10u");

        // 3 connections: GND→R1:0, R1:1→C1:0, C1:1→VCC
        assert_eq!(circuit.connections.len(), 3);
    }

    #[test]
    fn build_circuit_single_component_no_labels() {
        let tokens = tokenize("-R1(100)-");
        let circuit = build_circuit(&tokens);
        assert_eq!(circuit.components.len(), 1);
        assert!(circuit.connections.is_empty()); // no labels, so no connections
    }

    #[test]
    fn tokenize_multiple_wires() {
        let tokens = tokenize("--- ------ ----");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], Token::Wire));
        assert!(matches!(tokens[1], Token::Wire));
        assert!(matches!(tokens[2], Token::Wire));
    }
}
