use serde_json::Value;

/// Compare a pinned 1.3.13 native receipt with a newer build without editing
/// its historical bytes. Only engine markers change; musical data stays exact.
pub fn retarget_engine_versions(value: &mut Value) -> usize {
    match value {
        Value::Object(fields) => {
            let mut count = 0;
            if let Some(Value::String(version)) = fields.get_mut("engine_version") {
                if version == "1.3.13" {
                    *version = env!("CARGO_PKG_VERSION").to_owned();
                    count += 1;
                } else if let Some(suffix) = version.strip_prefix("1.3.13+") {
                    *version = format!("{}+{suffix}", env!("CARGO_PKG_VERSION"));
                    count += 1;
                }
            }
            for child in fields.values_mut() {
                count += retarget_engine_versions(child);
            }
            count
        }
        Value::Array(items) => items.iter_mut().map(retarget_engine_versions).sum(),
        _ => 0,
    }
}
