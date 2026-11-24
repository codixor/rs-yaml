use rs_yaml::safe_load;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r"
defaults: &defaults
  adapter: postgres

development:
  <<: *defaults
  database: dev_db
";

    let document = safe_load(input)?;
    println!("{document:#?}");
    Ok(())
}
