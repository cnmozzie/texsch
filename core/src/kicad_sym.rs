// ============================================================
// KiCad 10.0 S-Expression Symbol Parser
// ============================================================
//
// Parses .kicad_sym files, extracts pins (filtering hidden NC pins
// for rigid constraints), geometry (polylines etc.), and converts
// to ComponentSymbol for unified pipeline integration.

use std::collections::HashMap;

use crate::parser::{ComponentSymbol, DrawPrimitive, PinDirection, PinTemplate};

/// One entry in the symbol library: holds both the parsed [`ComponentSymbol`]
/// and the raw `.kicad_sym` file content so KiCad output can emit full
/// `lib_symbols` definitions without hardcoding individual files.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    /// Original `.kicad_sym` file content.
    pub raw_content: String,
    /// KiCad library prefix (e.g. `"Device"`, `"Amplifier_Operational"`).
    pub lib_prefix: String,
    /// The symbol name as written in the source file (e.g. `"R"`, `"OPA330xxD"`).
    pub sym_name_in_file: String,
}

/// Complete symbol library: fast lookup by short symbol name plus the raw
/// file entries needed for KiCad `lib_symbols` emission.
#[derive(Debug, Clone)]
pub struct LibraryBundle {
    /// Map from short symbol name (e.g. `"R"`) to parsed [`ComponentSymbol`].
    pub symbols: HashMap<String, ComponentSymbol>,
    /// Raw file entries for every loaded symbol (used by KiCad output).
    pub entries: Vec<LibraryEntry>,
}

// ============================================================
// Generic S-Expression types and parser
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn first_atom(&self) -> Option<&str> {
        match self {
            SExpr::List(items) => items.first()?.as_atom(),
            SExpr::Atom(s) => Some(s),
        }
    }

    /// Find the first child list whose first atom matches `tag`.
    pub fn find_child<'a>(&'a self, tag: &str) -> Option<&'a SExpr> {
        match self {
            SExpr::List(items) => items.iter().find(|item| {
                item.first_atom().map_or(false, |a| a == tag)
            }),
            _ => None,
        }
    }

    /// Find all child lists whose first atom matches `tag`.
    pub fn find_all_children<'a>(&'a self, tag: &str) -> Vec<&'a SExpr> {
        match self {
            SExpr::List(items) => items.iter().filter(|item| {
                item.first_atom().map_or(false, |a| a == tag)
            }).collect(),
            _ => vec![],
        }
    }

    /// Get the nth element (0-indexed) of a list.
    pub fn nth(&self, idx: usize) -> Option<&SExpr> {
        match self {
            SExpr::List(items) => items.get(idx),
            _ => None,
        }
    }
}

/// Parse a KiCad S-Expression string into a tree.
pub fn parse_sexpr(input: &str) -> Option<SExpr> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut stack: Vec<Vec<SExpr>> = Vec::new();
    let mut root: Option<SExpr> = None;

    while pos < chars.len() {
        match chars[pos] {
            '(' => {
                stack.push(Vec::new());
                pos += 1;
            }
            ')' => {
                let list = stack.pop()?;
                let sexpr = SExpr::List(list);
                match stack.last_mut() {
                    Some(current) => current.push(sexpr),
                    None => root = Some(sexpr),
                }
                pos += 1;
            }
            '"' => {
                pos += 1; // skip opening "
                let start = pos;
                while pos < chars.len() && chars[pos] != '"' {
                    pos += 1;
                }
                let atom: String = chars[start..pos].iter().collect();
                if pos < chars.len() {
                    pos += 1; // skip closing "
                }
                match stack.last_mut() {
                    Some(current) => current.push(SExpr::Atom(atom)),
                    None => return None, // atom outside list — invalid
                }
            }
            ch if ch.is_whitespace() => {
                pos += 1;
            }
            _ => {
                // bare atom — read until whitespace, '(' or ')'
                let start = pos;
                while pos < chars.len()
                    && !chars[pos].is_whitespace()
                    && chars[pos] != '('
                    && chars[pos] != ')'
                    && chars[pos] != '"'
                {
                    pos += 1;
                }
                let atom: String = chars[start..pos].iter().collect();
                match stack.last_mut() {
                    Some(current) => current.push(SExpr::Atom(atom)),
                    None => {} // top-level atoms are ignored
                }
            }
        }
    }

    root
}

// ============================================================
// Pin extraction
// ============================================================

#[derive(Debug, Clone)]
pub struct KiCadPin {
    pub number: String,
    pub name: String,
    pub pin_type: String,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub length: f64,
    pub hide: bool,
}

fn extract_pin(pin_expr: &SExpr) -> Option<KiCadPin> {
    let items = pin_expr.as_list()?;

    // (pin <type> <shape> (at <x> <y> <angle>) ...)
    if items.first()?.as_atom()? != "pin" {
        return None;
    }

    let pin_type = items.get(1)?.as_atom()?.to_string();
    // items[2] is shape ("line", "clock", etc.) — skip
    let at_expr = items.iter().find(|item| item.first_atom() == Some("at"))?;
    let at_items = at_expr.as_list()?;
    let x: f64 = at_items.get(1)?.as_atom()?.parse().ok()?;
    let y: f64 = at_items.get(2)?.as_atom()?.parse().ok()?;
    let angle: f64 = at_items.get(3)?.as_atom()?.parse().ok()?;

    let length_expr = items.iter().find(|item| item.first_atom() == Some("length"))?;
    let length: f64 = length_expr.as_list()?.get(1)?.as_atom()?.parse().ok()?;

    let name_expr = items.iter().find(|item| item.first_atom() == Some("name"))?;
    let name = extract_text_value(name_expr).unwrap_or_default();

    let number_expr = items.iter().find(|item| item.first_atom() == Some("number"))?;
    let number = extract_text_value(number_expr).unwrap_or_default();

    let hide = items.iter().any(|item| {
        item.first_atom() == Some("hide") && item.as_list().map_or(false, |h| {
            h.get(1).and_then(|v| v.as_atom()).map_or(false, |v| v == "yes")
        })
    });

    Some(KiCadPin { number, name, pin_type, x, y, angle, length, hide })
}

/// Extract the string value from expressions like `(name "V+")` or `(name "V+" (effects ...))`.
fn extract_text_value(expr: &SExpr) -> Option<String> {
    match expr {
        SExpr::List(items) => {
            if items.len() >= 2 {
                items[1].as_atom().map(|s| s.to_string())
            } else {
                None
            }
        }
        SExpr::Atom(s) => Some(s.clone()),
    }
}

// ============================================================
// Geometry extraction
// ============================================================

fn extract_polyline(poly_expr: &SExpr) -> Option<DrawPrimitive> {
    let items = poly_expr.as_list()?;
    if items.first()?.as_atom()? != "polyline" {
        return None;
    }

    let pts_expr = items.iter().find(|item| item.first_atom() == Some("pts"))?;
    let pts_items = pts_expr.as_list()?;

    let mut pts = Vec::new();
    // pts list: (pts (xy x1 y1) (xy x2 y2) ...)
    for pt in &pts_items[1..] {
        if pt.first_atom() == Some("xy") {
            let xy = pt.as_list()?;
            let x: f64 = xy.get(1)?.as_atom()?.parse().ok()?;
            let y: f64 = xy.get(2)?.as_atom()?.parse().ok()?;
            pts.push((x, y));
        }
    }

    let stroke_expr = items.iter().find(|item| item.first_atom() == Some("stroke"))?;
    let stroke_width = stroke_expr
        .as_list()?
        .iter()
        .find(|item| item.first_atom() == Some("width"))
        .and_then(|w| w.as_list()?.get(1)?.as_atom()?.parse::<f64>().ok())
        .unwrap_or(0.254);

    let fill_type = items
        .iter()
        .find(|item| item.first_atom() == Some("fill"))
        .and_then(|f| {
            f.as_list()?
                .iter()
                .find(|item| item.first_atom() == Some("type"))
                .and_then(|t| t.as_list()?.get(1)?.as_atom().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "none".to_string());

    Some(DrawPrimitive::Polyline { pts, stroke_width, fill_type })
}

fn extract_rectangle(rect_expr: &SExpr) -> Option<DrawPrimitive> {
    let items = rect_expr.as_list()?;
    if items.first()?.as_atom()? != "rectangle" {
        return None;
    }

    let parse_xy = |tag: &str| -> Option<(f64, f64)> {
        let e = items.iter().find(|item| item.first_atom() == Some(tag))?;
        let xy = e.as_list()?;
        Some((xy.get(1)?.as_atom()?.parse().ok()?, xy.get(2)?.as_atom()?.parse().ok()?))
    };

    let start = parse_xy("start")?;
    let end = parse_xy("end")?;

    let stroke_width = items.iter()
        .find(|item| item.first_atom() == Some("stroke"))
        .and_then(|s| s.as_list()?.iter().find(|i| i.first_atom() == Some("width")))
        .and_then(|w| w.as_list()?.get(1)?.as_atom()?.parse::<f64>().ok())
        .unwrap_or(0.254);

    let fill_type = items.iter()
        .find(|item| item.first_atom() == Some("fill"))
        .and_then(|f| {
            f.as_list()?.iter()
                .find(|i| i.first_atom() == Some("type"))
                .and_then(|t| t.as_list()?.get(1)?.as_atom().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "none".to_string());

    Some(DrawPrimitive::Rectangle { start, end, stroke_width, fill_type })
}

fn extract_circle(circle_expr: &SExpr) -> Option<DrawPrimitive> {
    let items = circle_expr.as_list()?;
    if items.first()?.as_atom()? != "circle" {
        return None;
    }

    let center_expr = items.iter().find(|item| item.first_atom() == Some("center"))?;
    let center_items = center_expr.as_list()?;
    let cx: f64 = center_items.get(1)?.as_atom()?.parse().ok()?;
    let cy: f64 = center_items.get(2)?.as_atom()?.parse().ok()?;

    let radius_expr = items.iter().find(|item| item.first_atom() == Some("radius"))?;
    let radius: f64 = radius_expr.as_list()?.get(1)?.as_atom()?.parse().ok()?;

    let stroke_width = items.iter()
        .find(|item| item.first_atom() == Some("stroke"))
        .and_then(|s| s.as_list()?.iter().find(|i| i.first_atom() == Some("width")))
        .and_then(|w| w.as_list()?.get(1)?.as_atom()?.parse::<f64>().ok())
        .unwrap_or(0.254);

    let fill_type = items.iter()
        .find(|item| item.first_atom() == Some("fill"))
        .and_then(|f| {
            f.as_list()?.iter()
                .find(|i| i.first_atom() == Some("type"))
                .and_then(|t| t.as_list()?.get(1)?.as_atom().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "none".to_string());

    Some(DrawPrimitive::Circle { center: (cx, cy), radius, stroke_width, fill_type })
}

fn extract_arc(arc_expr: &SExpr) -> Option<DrawPrimitive> {
    let items = arc_expr.as_list()?;
    if items.first()?.as_atom()? != "arc" {
        return None;
    }

    let parse_xy = |tag: &str| -> Option<(f64, f64)> {
        let e = items.iter().find(|item| item.first_atom() == Some(tag))?;
        let xy = e.as_list()?;
        Some((xy.get(1)?.as_atom()?.parse().ok()?, xy.get(2)?.as_atom()?.parse().ok()?))
    };

    let start = parse_xy("start")?;
    let mid = parse_xy("mid")?;
    let end = parse_xy("end")?;

    let stroke_width = items.iter()
        .find(|item| item.first_atom() == Some("stroke"))
        .and_then(|s| s.as_list()?.iter().find(|i| i.first_atom() == Some("width")))
        .and_then(|w| w.as_list()?.get(1)?.as_atom()?.parse::<f64>().ok())
        .unwrap_or(0.254);

    let fill_type = items.iter()
        .find(|item| item.first_atom() == Some("fill"))
        .and_then(|f| {
            f.as_list()?.iter()
                .find(|i| i.first_atom() == Some("type"))
                .and_then(|t| t.as_list()?.get(1)?.as_atom().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "none".to_string());

    Some(DrawPrimitive::Arc { start, mid, end, stroke_width, fill_type })
}

// ============================================================
// Full symbol extraction
// ============================================================

#[derive(Debug, Clone)]
pub struct KiCadSymbol {
    pub name: String,
    pub all_pins: Vec<KiCadPin>,
    pub visible_pins: Vec<KiCadPin>,
    pub draw_primitives: Vec<DrawPrimitive>,
    /// Reference property offset from symbol origin (KiCad Y-up, mm).
    pub ref_ki_x: f64,
    pub ref_ki_y: f64,
    pub ref_ki_angle: f64,
    /// Value property offset from symbol origin (KiCad Y-up, mm).
    pub val_ki_x: f64,
    pub val_ki_y: f64,
    pub val_ki_angle: f64,
}

/// Parse a single `(symbol "NAME" ...)` SExpr into a [`KiCadSymbol`].
fn extract_symbol(sym_expr: &SExpr) -> Option<KiCadSymbol> {
    let items = sym_expr.as_list()?;
    if items.first()?.as_atom()? != "symbol" {
        return None;
    }

    let name = items.get(1)?.as_atom()?.to_string();

    let mut all_pins = Vec::new();
    let mut draw_primitives = Vec::new();
    let mut ref_ki_x = 0.0f64;
    let mut ref_ki_y = 0.0f64;
    let mut ref_ki_angle = 0.0f64;
    let mut val_ki_x = 0.0f64;
    let mut val_ki_y = 0.0f64;
    let mut val_ki_angle = 0.0f64;

    fn parse_property_at(prop: &SExpr) -> (f64, f64, f64) {
        let at = prop.as_list()
            .and_then(|items| items.iter().find(|i| i.first_atom() == Some("at")));
        match at.and_then(|a| a.as_list()) {
            Some(at_items) if at_items.len() >= 4 => (
                at_items.get(1).and_then(|v| v.as_atom()).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                at_items.get(2).and_then(|v| v.as_atom()).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                at_items.get(3).and_then(|v| v.as_atom()).and_then(|s| s.parse().ok()).unwrap_or(0.0),
            ),
            _ => (0.0, 0.0, 0.0),
        }
    }

    // Recurse into all sub-nodes — including nested (symbol ...) sub-blocks
    for item in items.iter().skip(2) {
        match item.first_atom() {
            Some("pin") => {
                if let Some(pin) = extract_pin(item) {
                    all_pins.push(pin);
                }
            }
            Some("polyline") => {
                if let Some(dp) = extract_polyline(item) {
                    draw_primitives.push(dp);
                }
            }
            Some("rectangle") => {
                if let Some(dp) = extract_rectangle(item) {
                    draw_primitives.push(dp);
                }
            }
            Some("arc") => {
                if let Some(dp) = extract_arc(item) {
                    draw_primitives.push(dp);
                }
            }
            Some("circle") => {
                if let Some(dp) = extract_circle(item) {
                    draw_primitives.push(dp);
                }
            }
            Some("symbol") => {
                // Recurse into sub-symbol (e.g., "OPA330xxD_0_1", "OPA330xxD_1_1")
                if let Some(sub) = extract_symbol(item) {
                    all_pins.extend(sub.all_pins);
                    draw_primitives.extend(sub.draw_primitives);
                }
            }
            Some("property") => {
                let prop_name = item.as_list()
                    .and_then(|l| l.get(1))
                    .and_then(|v| v.as_atom())
                    .unwrap_or("");
                let (px, py, pa) = parse_property_at(item);
                match prop_name {
                    "Reference" => { ref_ki_x = px; ref_ki_y = py; ref_ki_angle = pa; }
                    "Value" => { val_ki_x = px; val_ki_y = py; val_ki_angle = pa; }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let visible_pins: Vec<KiCadPin> = all_pins.iter()
        .filter(|p| !p.hide)
        .cloned()
        .collect();

    Some(KiCadSymbol {
        name, all_pins, visible_pins, draw_primitives,
        ref_ki_x, ref_ki_y, ref_ki_angle,
        val_ki_x, val_ki_y, val_ki_angle,
    })
}

// ============================================================
// Public API
// ============================================================

/// Parse a .kicad_sym file and return all top-level symbols.
pub fn parse_kicad_sym(input: &str) -> Vec<KiCadSymbol> {
    let root = match parse_sexpr(input) {
        Some(r) => r,
        None => return vec![],
    };

    // root is (kicad_symbol_lib ...)
    // Find all (symbol ...) children
    let mut symbols = Vec::new();
    for item in root.as_list().unwrap_or(&[]) {
        if item.first_atom() == Some("symbol") {
            if let Some(sym) = extract_symbol(item) {
                symbols.push(sym);
            }
        }
    }

    symbols
}

/// Load a symbol from an embedded KiCad symbol file, assigning the given
/// `lib_prefix` (e.g. `"Device"`, `"Amplifier_Operational"`) so the
/// resulting [`ComponentSymbol`] has both a short `symbol_name` and a
/// fully-qualified `lib_id`.
pub fn load_symbol_from_file(file_contents: &str, lib_prefix: &str) -> ComponentSymbol {
    let symbols = parse_kicad_sym(file_contents);
    let mut sym = to_component_symbol(&symbols[0]);
    // lib_id = "Device:R", symbol_name stays as "R"
    sym.lib_id = format!("{}:{}", lib_prefix, sym.symbol_name);
    sym
}

/// Load the OPA330xxD symbol from the embedded KiCad symbol file.
pub fn load_opa330xxd_symbol() -> ComponentSymbol {
    load_symbol_from_file(
        include_str!("../library/Amplifier_Operational/opa330xxd.kicad_sym"),
        "Amplifier_Operational",
    )
}

/// Load the Device:R (Resistor) symbol.
pub fn load_r_symbol() -> ComponentSymbol {
    load_symbol_from_file(
        include_str!("../library/Device/r.kicad_sym"),
        "Device",
    )
}

/// Load the Device:L (Inductor) symbol.
pub fn load_l_symbol() -> ComponentSymbol {
    load_symbol_from_file(
        include_str!("../library/Device/l.kicad_sym"),
        "Device",
    )
}

/// Load the Device:C (Capacitor) symbol.
pub fn load_c_symbol() -> ComponentSymbol {
    load_symbol_from_file(
        include_str!("../library/Device/c.kicad_sym"),
        "Device",
    )
}

/// Load all symbols from a library directory on the filesystem.
///
/// The directory is expected to have one subdirectory per KiCad library
/// (e.g. `Device/`, `Amplifier_Operational/`).  Each subdirectory contains
/// `.kicad_sym` files.  The subdirectory name is used as the lib prefix;
/// the symbol name embedded in the file is the short key.
///
/// Returns a [`LibraryBundle`] with both the parsed symbol map and raw file
/// entries for KiCad `lib_symbols` emission.
pub fn load_symbol_library(dir_path: &str) -> LibraryBundle {
    let mut symbols: HashMap<String, ComponentSymbol> = HashMap::new();
    let mut entries: Vec<LibraryEntry> = Vec::new();
    let dir = match std::fs::read_dir(dir_path) {
        Ok(d) => d,
        Err(_) => return LibraryBundle { symbols, entries },
    };

    for entry in dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let lib_prefix = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let sub_dir = match std::fs::read_dir(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        for file_entry in sub_dir.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().map_or(true, |e| e != "kicad_sym") {
                continue;
            }

            let content = match std::fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let parsed = parse_kicad_sym(&content);
            for sym in &parsed {
                let mut comp = to_component_symbol(sym);
                comp.lib_id = format!("{}:{}", lib_prefix, comp.symbol_name);
                entries.push(LibraryEntry {
                    raw_content: content.clone(),
                    lib_prefix: lib_prefix.clone(),
                    sym_name_in_file: comp.symbol_name.clone(),
                });
                symbols.insert(comp.symbol_name.clone(), comp);
            }
        }
    }

    LibraryBundle { symbols, entries }
}

// Include the auto-generated file list from build.rs
include!(concat!(env!("OUT_DIR"), "/builtin_library_generated.rs"));

/// Build a symbol library from the built-in (compile-time embedded) KiCad files.
///
/// Symbols are auto-discovered by `build.rs` scanning `library/` subdirectories.
/// Adding a new `.kicad_sym` file requires **zero Rust code changes** — just
/// drop the file into the appropriate `library/<LibName>/` subdirectory.
///
/// Returns a [`LibraryBundle`] that includes both the parsed symbol map and the
/// raw file entries needed for KiCad `lib_symbols` emission.
pub fn load_builtin_library() -> LibraryBundle {
    let mut symbols: HashMap<String, ComponentSymbol> = HashMap::new();
    let mut entries: Vec<LibraryEntry> = Vec::new();

    for (raw_content, lib_prefix) in builtin_entries() {
        let parsed = parse_kicad_sym(raw_content);
        for sym in &parsed {
            let mut comp = to_component_symbol(sym);
            comp.lib_id = format!("{}:{}", lib_prefix, comp.symbol_name);
            entries.push(LibraryEntry {
                raw_content: raw_content.to_string(),
                lib_prefix: lib_prefix.to_string(),
                sym_name_in_file: comp.symbol_name.clone(),
            });
            symbols.insert(comp.symbol_name.clone(), comp);
        }
    }

    LibraryBundle { symbols, entries }
}

/// KiCad angle → [`PinDirection`].
/// Mapping: 0→Left, 180→Right, 270→Up, 90→Down.
fn angle_to_direction(angle: f64) -> PinDirection {
    // Normalize to [0, 360)
    let a = ((angle % 360.0) + 360.0) % 360.0;
    if (a - 0.0).abs() < 1.0 {
        PinDirection::Left
    } else if (a - 180.0).abs() < 1.0 {
        PinDirection::Right
    } else if (a - 270.0).abs() < 1.0 {
        PinDirection::Up
    } else if (a - 90.0).abs() < 1.0 {
        PinDirection::Down
    } else {
        // Fallback: choose based on nearest cardinal
        if a < 45.0 || a >= 315.0 {
            PinDirection::Left
        } else if a < 135.0 {
            PinDirection::Down
        } else if a < 225.0 {
            PinDirection::Right
        } else {
            PinDirection::Up
        }
    }
}

/// Grid unit size in mm (2.54mm = 0.1 inch = 1 KiCad grid unit).
pub const GRID_UNIT_MM: f64 = 2.54;

/// Convert a parsed [`KiCadSymbol`] into a [`ComponentSymbol`] suitable
/// for the rigid template matching pipeline.
///
/// * The **leftmost-then-topmost** visible pin is used as the anchor
///   (min X, tiebreak by max Y). This gives the natural reference pin.
/// * `rel_phys` — actual physical offset in SVG px, derived from mm
///   coordinates.  These are the **rigid spacing constraints** used by
///   the DAG solver.
/// * `rel_grid` — **compact** grid index.  All unique X (and Y)
///   coordinates are collected from visible pins, sorted, and each pin
///   receives the index of its coordinate in that sorted list.  This
///   gives the tightest possible ASCII grid layout with no wasted
///   intermediate rows or columns.
/// * [`DrawPrimitive`] coordinates remain in physical grid units
///   (mm / 2.54) so SVG scaling by `CELL_W` / `CELL_H` works correctly.
pub fn to_component_symbol(sym: &KiCadSymbol) -> ComponentSymbol {
    // Anchor: leftmost visible pin (min X), tiebreak by topmost (max Y).
    let anchor = sym.visible_pins.iter()
        .min_by(|a, b| {
            a.x.partial_cmp(&b.x).unwrap()
                .then_with(|| b.y.partial_cmp(&a.y).unwrap())
        })
        .expect("no visible pins in symbol");

    let ax = anchor.x;
    let ay = anchor.y;
    let anchor_pin_num: usize = anchor.number.parse().unwrap_or(0);

    // --- compact grid: sort unique X and Y coordinates ----------------
    let mut xs: Vec<f64> = sym.visible_pins.iter().map(|p| p.x).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup_by(|a, b| (*a - *b).abs() < 0.001);

    let mut ys: Vec<f64> = sym.visible_pins.iter().map(|p| p.y).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.001);

    fn x_to_col(x: f64, xs: &[f64]) -> usize {
        xs.iter().position(|&v| (v - x).abs() < 0.001).unwrap()
    }

    // Y in KiCad increases upward; compact row 0 = topmost (largest Y).
    fn y_to_row(y: f64, ys: &[f64]) -> usize {
        ys.iter().rev().position(|&v| (v - y).abs() < 0.001).unwrap()
    }

    let anchor_col = x_to_col(ax, &xs);
    let anchor_row = y_to_row(ay, &ys);

    let pins: Vec<PinTemplate> = sym.visible_pins.iter().map(|p| {
        let dx = p.x - ax;
        let dy = p.y - ay;
        let rel_grid_col = x_to_col(p.x, &xs) as i32 - anchor_col as i32;
        let rel_grid_row = y_to_row(p.y, &ys) as i32 - anchor_row as i32;
        let dir = angle_to_direction(p.angle);

        PinTemplate {
            pin_num: p.number.parse().unwrap_or(0),
            name: p.name.clone(),
            dir,
            rel_grid_row,
            rel_grid_col,
            rel_phys_x: dx,  // raw mm — DAG solver now works in mm
            rel_phys_y: -dy, // raw mm (Y-flipped from KiCad convention)
            pin_length_mm: p.length,
        }
    }).collect();

    // Ensure anchor pin is first in the pins list
    let anchor_idx = pins.iter().position(|p| p.pin_num == anchor_pin_num).unwrap_or(0);
    let mut pins = pins;
    if anchor_idx != 0 {
        pins.swap(0, anchor_idx);
    }

    let to_grid = |mx: f64, my: f64| -> (f64, f64) {
        ((mx - ax) / GRID_UNIT_MM, -(my - ay) / GRID_UNIT_MM)
    };

    let draw_primitives: Vec<DrawPrimitive> = sym.draw_primitives.iter().map(|dp| {
        match dp {
            DrawPrimitive::Polyline { pts, stroke_width, fill_type } => {
                let grid_pts: Vec<(f64, f64)> = pts.iter()
                    .map(|&(px, py)| to_grid(px, py)).collect();
                DrawPrimitive::Polyline {
                    pts: grid_pts,
                    stroke_width: *stroke_width,
                    fill_type: fill_type.clone(),
                }
            }
            DrawPrimitive::Rectangle { start, end, stroke_width, fill_type } => {
                DrawPrimitive::Rectangle {
                    start: to_grid(start.0, start.1),
                    end: to_grid(end.0, end.1),
                    stroke_width: *stroke_width,
                    fill_type: fill_type.clone(),
                }
            }
            DrawPrimitive::Arc { start, mid, end, stroke_width, fill_type } => {
                DrawPrimitive::Arc {
                    start: to_grid(start.0, start.1),
                    mid: to_grid(mid.0, mid.1),
                    end: to_grid(end.0, end.1),
                    stroke_width: *stroke_width,
                    fill_type: fill_type.clone(),
                }
            }
            DrawPrimitive::Circle { center, radius, stroke_width, fill_type } => {
                DrawPrimitive::Circle {
                    center: to_grid(center.0, center.1),
                    radius: radius / GRID_UNIT_MM,
                    stroke_width: *stroke_width,
                    fill_type: fill_type.clone(),
                }
            }
        }
    }).collect();

    let all_pin_numbers: Vec<usize> = sym.all_pins.iter()
        .map(|p| p.number.parse().unwrap_or(0))
        .collect();

    let feature_pin_a: Option<usize>;
    let feature_pin_b: Option<usize>;
    if sym.visible_pins.len() == 2 {
        feature_pin_a = Some(sym.visible_pins[0].number.parse().unwrap_or(0));
        feature_pin_b = Some(sym.visible_pins[1].number.parse().unwrap_or(0));
    } else if sym.name == "OPA330xxD" {
        feature_pin_a = Some(2);
        feature_pin_b = Some(6);
    } else {
        feature_pin_a = None;
        feature_pin_b = None;
    }

    ComponentSymbol {
        symbol_name: sym.name.clone(),
        lib_id: sym.name.clone(), // overwritten by load_symbol_from_file / load_symbol_library
        pins,
        draw_primitives,
        all_pin_numbers,
        anchor_ki_x: anchor.x,
        anchor_ki_y: anchor.y,
        feature_pin_a,
        feature_pin_b,
        ref_ki_x: sym.ref_ki_x,
        ref_ki_y: sym.ref_ki_y,
        ref_ki_angle: sym.ref_ki_angle,
        val_ki_x: sym.val_ki_x,
        val_ki_y: sym.val_ki_y,
        val_ki_angle: sym.val_ki_angle,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn read_test_file() -> String {
        std::fs::read_to_string("/home/mozzie/texsch/core/library/Amplifier_Operational/opa330xxd.kicad_sym")
            .expect("failed to read opa330xxd.kicad_sym")
    }

    #[test]
    fn parse_sexpr_simple_atom_list() {
        let result = parse_sexpr("(a \"b\" 123)").unwrap();
        assert_eq!(
            result,
            SExpr::List(vec![
                SExpr::Atom("a".to_string()),
                SExpr::Atom("b".to_string()),
                SExpr::Atom("123".to_string()),
            ])
        );
    }

    #[test]
    fn parse_sexpr_nested_lists() {
        let result = parse_sexpr("(outer (inner 1) (inner 2))").unwrap();
        let items = result.as_list().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].as_atom().unwrap(), "outer");
        assert_eq!(items[1].first_atom().unwrap(), "inner");
        assert_eq!(items[2].first_atom().unwrap(), "inner");
    }

    #[test]
    fn parse_opa330xxd_basic_structure() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        assert_eq!(symbols.len(), 1, "expected 1 top-level symbol");

        let sym = &symbols[0];
        assert_eq!(sym.name, "OPA330xxD");
    }

    #[test]
    fn opa330xxd_all_pins_count() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        assert_eq!(sym.all_pins.len(), 8, "expected 8 pins total");
    }

    #[test]
    fn opa330xxd_hidden_pins_filtered() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        // Pins 1, 5, 8 are hidden NC pins
        assert_eq!(sym.visible_pins.len(), 5, "expected 5 visible pins (2,3,4,6,7)");

        let visible_numbers: Vec<usize> = sym.visible_pins.iter()
            .map(|p| p.number.parse::<usize>().unwrap())
            .collect();
        assert_eq!(visible_numbers, vec![2, 3, 4, 6, 7]);

        let hidden_numbers: Vec<usize> = sym.all_pins.iter()
            .filter(|p| p.hide)
            .map(|p| p.number.parse::<usize>().unwrap())
            .collect();
        assert_eq!(hidden_numbers, vec![1, 5, 8]);
    }

    #[test]
    fn opa330xxd_pin_2_coordinates() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let pin2 = sym.all_pins.iter().find(|p| p.number == "2").unwrap();
        assert!((pin2.x + 7.62).abs() < 0.01, "pin2 x should be -7.62, got {}", pin2.x);
        assert!((pin2.y + 2.54).abs() < 0.01, "pin2 y should be -2.54, got {}", pin2.y);
        assert!((pin2.angle - 0.0).abs() < 0.01, "pin2 angle should be 0");
        assert!(!pin2.hide, "pin2 should be visible");
    }

    #[test]
    fn opa330xxd_pin_3_coordinates() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let pin3 = sym.all_pins.iter().find(|p| p.number == "3").unwrap();
        assert!((pin3.x + 7.62).abs() < 0.01, "pin3 x should be -7.62, got {}", pin3.x);
        assert!((pin3.y - 2.54).abs() < 0.01, "pin3 y should be 2.54, got {}", pin3.y);
        assert!((pin3.angle - 0.0).abs() < 0.01, "pin3 angle should be 0");
        assert!(!pin3.hide, "pin3 should be visible");
    }

    #[test]
    fn opa330xxd_pin_6_coordinates() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let pin6 = sym.all_pins.iter().find(|p| p.number == "6").unwrap();
        assert!((pin6.x - 7.62).abs() < 0.01, "pin6 x should be 7.62, got {}", pin6.x);
        assert!((pin6.y - 0.0).abs() < 0.01, "pin6 y should be 0, got {}", pin6.y);
        assert!((pin6.angle - 180.0).abs() < 0.01, "pin6 angle should be 180");
        assert!(!pin6.hide, "pin6 should be visible");
    }

    #[test]
    fn opa330xxd_anchor_is_pin_3() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        // User requirement: Pin 3 (+) is the anchor
        // We use the first visible pin sorted so that Pin 3 comes first
        let comp_sym = to_component_symbol(sym);
        let anchor = &comp_sym.pins[0];
        assert_eq!(anchor.pin_num, 3, "anchor should be pin 3");
        assert_eq!(anchor.name, "+");
        assert_eq!(anchor.rel_grid_row, 0);
        assert_eq!(anchor.rel_grid_col, 0);
    }

    #[test]
    fn opa330xxd_pin2_relative_to_anchor() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let comp_sym = to_component_symbol(sym);
        let pin2 = comp_sym.pins.iter().find(|p| p.pin_num == 2).unwrap();

        // Pin 2 is 2 rows below anchor (pin 3), same column
        assert_eq!(pin2.rel_grid_row, 2, "pin2 rel_grid_row should be 2, got {}", pin2.rel_grid_row);
        assert_eq!(pin2.rel_grid_col, 0, "pin2 rel_grid_col should be 0, got {}", pin2.rel_grid_col);
        assert_eq!(pin2.dir, PinDirection::Left, "pin2 direction should be Left, got {:?}", pin2.dir);
    }

    #[test]
    fn opa330xxd_pin6_relative_to_anchor() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let comp_sym = to_component_symbol(sym);
        let pin6 = comp_sym.pins.iter().find(|p| p.pin_num == 6).unwrap();

        // Pin 6 is 1 row below anchor, 2 cols right (compact grid)
        assert_eq!(pin6.rel_grid_row, 1, "pin6 rel_grid_row should be 1, got {}", pin6.rel_grid_row);
        assert_eq!(pin6.rel_grid_col, 2, "pin6 rel_grid_col should be 2 (compact), got {}", pin6.rel_grid_col);
        assert_eq!(pin6.dir, PinDirection::Right, "pin6 direction should be Right, got {:?}", pin6.dir);
    }

    #[test]
    fn opa330xxd_has_polyline_geometry() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        assert!(!sym.draw_primitives.is_empty(), "should have at least one polyline");
        // The triangle polyline has 4 points (closed triangle with return to start)
        let poly = &sym.draw_primitives[0];
        match poly {
            DrawPrimitive::Polyline { pts, fill_type, .. } => {
                assert_eq!(pts.len(), 4, "triangle should have 4 points");
                assert_eq!(fill_type, "background");
            }
            _ => {}
        }
    }

    #[test]
    fn opa330xxd_component_symbol_draw_primitives() {
        let input = read_test_file();
        let symbols = parse_kicad_sym(&input);
        let sym = &symbols[0];

        let comp_sym = to_component_symbol(sym);
        assert!(!comp_sym.draw_primitives.is_empty(),
            "ComponentSymbol should have draw primitives");

        // Verify the first primitive has finite coordinates
        match &comp_sym.draw_primitives[0] {
            DrawPrimitive::Polyline { pts, .. } => {
                for &(gx, gy) in pts {
                    assert!(gx.is_finite());
                    assert!(gy.is_finite());
                }
            }
            DrawPrimitive::Rectangle { start, end, .. } => {
                assert!(start.0.is_finite() && start.1.is_finite());
                assert!(end.0.is_finite() && end.1.is_finite());
            }
            DrawPrimitive::Arc { start, mid, end, .. } => {
                assert!(start.0.is_finite() && start.1.is_finite());
                assert!(mid.0.is_finite() && mid.1.is_finite());
                assert!(end.0.is_finite() && end.1.is_finite());
            }
            DrawPrimitive::Circle { center, radius, .. } => {
                assert!(center.0.is_finite() && center.1.is_finite());
                assert!(radius.is_finite());
            }
        }
    }

    #[test]
    fn angle_to_direction_mapping() {
        assert_eq!(angle_to_direction(0.0), PinDirection::Left);
        assert_eq!(angle_to_direction(180.0), PinDirection::Right);
        assert_eq!(angle_to_direction(270.0), PinDirection::Up);
        assert_eq!(angle_to_direction(90.0), PinDirection::Down);
    }

    /// End-to-end: parse KiCad file, render SVG with draw_primitives.
    #[test]
    fn e2e_svg_renders_kicad_draw_primitives() {
        let comp_sym = load_opa330xxd_symbol();

        // Build a minimal MatchedComponent with the parsed symbol data
        let matched = vec![crate::parser::MatchedComponent {
            refdes: "U1".to_string(),
            symbol_name: comp_sym.symbol_name.clone(),
            lib_id: comp_sym.lib_id.clone(),
            pins: comp_sym.pins.iter().map(|pt| {
                crate::parser::MatchedPin {
                    pin_num: pt.pin_num,
                    name: pt.name.clone(),
                    dir: pt.dir,
                    grid_row: (2i32 + pt.rel_grid_row) as usize,
                    grid_col: (1i32 + pt.rel_grid_col) as usize,
                    rel_phys_x: pt.rel_phys_x,
                    rel_phys_y: pt.rel_phys_y,
                    tmpl_phys_x: pt.rel_phys_x,
                    tmpl_phys_y: pt.rel_phys_y,
                    tmpl_dir: pt.dir,
                    pin_length_mm: pt.pin_length_mm,
                }
            }).collect(),
            anchor_grid_row: 2,
            anchor_grid_col: 1,
            draw_primitives: comp_sym.draw_primitives.clone(),
            all_pin_numbers: comp_sym.all_pin_numbers.clone(),
            anchor_ki_x: comp_sym.anchor_ki_x,
            anchor_ki_y: comp_sym.anchor_ki_y,
            angle: 0.0,
            pin_ki_x: vec![],
            pin_ki_y: vec![],
            ref_ki_x: comp_sym.ref_ki_x,
            ref_ki_y: comp_sym.ref_ki_y,
            ref_ki_angle: comp_sym.ref_ki_angle,
            val_ki_x: comp_sym.val_ki_x,
            val_ki_y: comp_sym.val_ki_y,
            val_ki_angle: comp_sym.val_ki_angle,
        }];

        let col_x: Vec<f64> = (0..=10).map(|i| 2.54 + i as f64 * 2.54).collect();
        let row_y: Vec<f64> = (0..=8).map(|i| 2.54 + i as f64 * 2.54).collect();

        let svg = crate::svg::generate_step3(
            &[],     // no schematic nodes
            &[],     // no wires
            col_x.as_slice(),
            row_y.as_slice(),
            &matched,
        );

        // Verify SVG contains the symbol body (polygon from draw_primitives)
        assert!(svg.contains("<polygon"), "SVG should contain polygon from draw_primitives");
        assert!(svg.contains("rgba(255,255,180,0.25)"), "SVG should have light yellow fill");
        assert!(svg.contains("#8B0000"), "SVG should have dark red stroke");
        // Verify pin labels
        assert!(svg.contains("U1:3(+)"), "SVG should have pin 3 label");
        assert!(svg.contains("U1:2(-)"), "SVG should have pin 2 label");
        assert!(svg.contains("U1:6"), "SVG should have pin 6 label");
    }

    /// End-to-end: parse KiCad file, generate KiCad output with full pin list.
    #[test]
    fn e2e_kicad_output_uses_parsed_symbol() {
        let comp_sym = load_opa330xxd_symbol();

        // Build a matched component
        let matched = vec![crate::parser::MatchedComponent {
            refdes: "U1".to_string(),
            symbol_name: comp_sym.symbol_name.clone(),
            lib_id: comp_sym.lib_id.clone(),
            pins: comp_sym.pins.iter().map(|pt| {
                crate::parser::MatchedPin {
                    pin_num: pt.pin_num,
                    name: pt.name.clone(),
                    dir: pt.dir,
                    grid_row: (2i32 + pt.rel_grid_row) as usize,
                    grid_col: (1i32 + pt.rel_grid_col) as usize,
                    rel_phys_x: pt.rel_phys_x,
                    rel_phys_y: pt.rel_phys_y,
                    tmpl_phys_x: pt.rel_phys_x,
                    tmpl_phys_y: pt.rel_phys_y,
                    tmpl_dir: pt.dir,
                    pin_length_mm: pt.pin_length_mm,
                }
            }).collect(),
            anchor_grid_row: 2,
            anchor_grid_col: 1,
            draw_primitives: comp_sym.draw_primitives.clone(),
            all_pin_numbers: comp_sym.all_pin_numbers.clone(),
            anchor_ki_x: comp_sym.anchor_ki_x,
            anchor_ki_y: comp_sym.anchor_ki_y,
            angle: 0.0,
            pin_ki_x: vec![],
            pin_ki_y: vec![],
            ref_ki_x: comp_sym.ref_ki_x,
            ref_ki_y: comp_sym.ref_ki_y,
            ref_ki_angle: comp_sym.ref_ki_angle,
            val_ki_x: comp_sym.val_ki_x,
            val_ki_y: comp_sym.val_ki_y,
            val_ki_angle: comp_sym.val_ki_angle,
        }];

        let col_x: Vec<f64> = (0..=10).map(|i| 2.54 + i as f64 * 2.54).collect();
        let row_y: Vec<f64> = (0..=8).map(|i| 2.54 + i as f64 * 2.54).collect();

        let kicad = crate::kicad::generate_step3(
            &[],            // no labels
            &[],            // no nodes
            col_x.as_slice(),
            row_y.as_slice(),
            &matched,
            "",             // no input text
            &[],            // no lib entries (instance test only)
        );

        // Verify KiCad output references the symbol with library prefix
        assert!(kicad.contains("Amplifier_Operational:OPA330xxD"),
            "KiCad should reference Amplifier_Operational:OPA330xxD symbol");
        assert!(kicad.contains("(lib_id \"Amplifier_Operational:OPA330xxD\")"),
            "KiCad should have lib_id");

        // Verify ALL 8 pins are included (including hidden NC pins)
        for pin_num in &[1, 2, 3, 4, 5, 6, 7, 8] {
            let pin_str = format!("(pin \"{}\"", pin_num);
            assert!(kicad.contains(&pin_str),
                "KiCad should include pin {}", pin_num);
        }

        // Verify Reference property
        assert!(kicad.contains("(property \"Reference\" \"U1\""),
            "KiCad should have Reference U1");
    }

    /// Verify that [`load_symbol_library`] correctly scans the library
    /// directory and registers all symbols with proper lib_ids.
    #[test]
    fn load_symbol_library_from_directory() {
        let bundle = load_symbol_library("/home/mozzie/texsch/core/library");

        assert!(bundle.symbols.len() >= 4, "expected at least 4 symbols, got {}", bundle.symbols.len());
        assert!(bundle.entries.len() >= 4, "expected at least 4 raw entries");

        // Device symbols
        let r = bundle.symbols.get("R").expect("R not found");
        assert_eq!(r.lib_id, "Device:R");
        assert_eq!(r.pins.len(), 2);
        assert!(!r.draw_primitives.is_empty(), "R should have rectangle from KiCad");

        let c = bundle.symbols.get("C").expect("C not found");
        assert_eq!(c.lib_id, "Device:C");
        assert_eq!(c.pins.len(), 2);

        let l = bundle.symbols.get("L").expect("L not found");
        assert_eq!(l.lib_id, "Device:L");
        assert_eq!(l.pins.len(), 2);

        // Amplifier_Operational symbol
        let opa = bundle.symbols.get("OPA330xxD").expect("OPA330xxD not found");
        assert_eq!(opa.lib_id, "Amplifier_Operational:OPA330xxD");
        assert_eq!(opa.pins.len(), 5, "OPA330xxD should have 5 visible pins");
        assert!(!opa.draw_primitives.is_empty());
    }

    /// Verify that [`load_builtin_library`] produces the same keys as the
    /// filesystem loader (for WASM compatibility).
    #[test]
    fn load_builtin_library_has_all_symbols() {
        let bundle = load_builtin_library();

        assert!(bundle.symbols.contains_key("R"));
        assert!(bundle.symbols.contains_key("C"));
        assert!(bundle.symbols.contains_key("L"));
        assert!(bundle.symbols.contains_key("OPA330xxD"));

        assert_eq!(bundle.symbols["R"].lib_id, "Device:R");
        assert_eq!(bundle.symbols["OPA330xxD"].lib_id, "Amplifier_Operational:OPA330xxD");

        assert!(bundle.entries.len() >= 6); // grows as new symbols are added
        // New Connector symbols should be auto-discovered
        assert!(bundle.symbols.contains_key("Conn_Coaxial"),
            "Conn_Coaxial should be auto-discovered from library/Connector/");
        assert!(bundle.symbols.contains_key("Conn_01x03_Socket"),
            "Conn_01x03_Socket should be auto-discovered from library/Connector/");
        assert_eq!(bundle.symbols["Conn_Coaxial"].lib_id, "Connector:Conn_Coaxial");
        assert_eq!(bundle.symbols["Conn_01x03_Socket"].lib_id, "Connector:Conn_01x03_Socket");
    }
}

