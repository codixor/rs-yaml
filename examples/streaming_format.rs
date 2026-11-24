#[cfg(feature = "streaming")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use rs_yaml::EmitterConfig;
    use rs_yaml::streaming::format_streaming;

    let input = "version: 1.0\nitems:\n  - one\n  - two\n";
    let output = format_streaming(input, &EmitterConfig::default())?;
    print!("{output}");
    Ok(())
}

#[cfg(not(feature = "streaming"))]
fn main() {
    eprintln!("enable the `streaming` feature to run this example");
}
