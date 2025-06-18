use std::fs;
use sp1_sdk::{ProverClient, SP1Stdin};
use anyhow::Result;

fn main() -> Result<()> {
    // Setup logging
    sp1_sdk::utils::setup_logger();
    
    // Read the ELF file
    let elf = fs::read("../my.elf")?;
    
    // Create empty stdin (RISC-V arch tests don't need input)
    let stdin = SP1Stdin::new();
    
    // Create ProverClient and execute
    let client = ProverClient::from_env();
    let (public_values, execution_report) = client.execute(&elf, &stdin).run()?;
    
    println!("✓ Execution completed successfully!");
    println!("Total instruction count: {}", execution_report.total_instruction_count());
    println!("Total syscall count: {}", execution_report.total_syscall_count());
    println!("Public values length: {}", public_values.buffer.len());
    
    // Output signatures/public values for RISC-V arch test validation
    if !public_values.buffer.is_empty() {
        println!("Public values (hex): {}", hex::encode(&public_values.buffer));
    }
    
    Ok(())
}
