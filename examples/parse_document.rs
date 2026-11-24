use rs_yaml::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "name: yaml\nfeatures:\n  - parse\n  - emit\n";
    let document = Parser::parse_str(input)?;
    println!("{document:#?}");
    Ok(())
}
