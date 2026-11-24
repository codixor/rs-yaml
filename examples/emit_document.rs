use rs_yaml::{Emitter, Map, ScalarOwned, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = Map::new();
    document.insert(
        Value::Value(ScalarOwned::String("name".to_owned())),
        Value::Value(ScalarOwned::String("yaml".to_owned())),
    );
    document.insert(
        Value::Value(ScalarOwned::String("enabled".to_owned())),
        Value::Value(ScalarOwned::Boolean(true)),
    );

    let output = Emitter::emit_str(&Value::Mapping(document))?;
    print!("{output}");
    Ok(())
}
