use rs_yaml::{Emitter, EmitterConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = "title: example\ndescription: |\n  first line\n  second line\n";
    let output =
        Emitter::format_with_config(input, &EmitterConfig::new().with_explicit_start(true))?;
    print!("{output}");
    Ok(())
}
