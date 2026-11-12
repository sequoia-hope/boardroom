//! Prints the intent file's JSON Schema, generated from the types:
//! `cargo run -p intent --example schema > crates/intent/schema/intent.schema.json`.

fn main() {
    println!("{}", serde_json::to_string_pretty(&intent::json_schema()).expect("schema serialises"));
}
