use sp1_sdk::SP1Stdin;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = SP1Stdin::new();
    let serialized = bincode::serialize(&stdin)?;
    std::fs::write("empty_stdin.bin", serialized)?;
    println!("Created empty_stdin.bin with {} bytes", serialized.len());
    Ok(())
}
