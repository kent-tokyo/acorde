//! Read score JSON on stdin and write a reviewable free-bass candidate/result to stdout.
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let score: acorde_core::Score = serde_json::from_str(&input)?;
    let result = acorde_core::split_for_free_bass(&score, acorde_core::FreeBassOptions::default());
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
