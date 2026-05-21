fn main() {
    let input = "GND --- -R1(24.9)- --- -C1(10u)- --- VCC";
    let result = texsch::compile(input);
    println!("=== SVG ===\n{}\n", result.svg);
    println!("=== KiCad ===\n{}", result.kicad_sch);
    std::fs::write("example_output.svg", &result.svg).unwrap();
    std::fs::write("example_output.kicad_sch", &result.kicad_sch).unwrap();
    eprintln!("Wrote example_output.svg and example_output.kicad_sch");
}
