use std::time::{Duration, Instant};

use clap::{command, Parser};
use p3_baby_bear::BabyBear;
use sp1_core_executor::{Executor, ExecutorMode, Program};
use sp1_core_machine::shape::CoreShapeConfig;
use sp1_sdk::{self, SP1Stdin};
use sp1_stark::SP1ProverOpts;

// Add ELF parsing imports
use elf::{abi::STT_OBJECT, endian::AnyEndian, ElfBytes};

#[derive(Parser, Clone)]
#[command(about = "Evaluate the performance of SP1 on programs.")]
struct PerfArgs {
    /// The program to evaluate.
    #[arg(short, long)]
    pub program: String,

    /// The input to the program being evaluated.
    #[arg(short, long)]
    pub stdin: String,

    /// The executor mode to use.
    #[arg(short, long)]
    pub executor_mode: ExecutorMode,

    /// Test signatures output file.
    #[arg(long)]
    pub signatures: Option<String>,
}

#[derive(Default, Debug, Clone)]
#[allow(dead_code)]
struct PerfResult {
    pub cycles: u64,
    pub execution_duration: Duration,
    pub prove_core_duration: Duration,
    pub verify_core_duration: Duration,
    pub compress_duration: Duration,
    pub verify_compressed_duration: Duration,
    pub shrink_duration: Duration,
    pub verify_shrink_duration: Duration,
    pub wrap_duration: Duration,
    pub verify_wrap_duration: Duration,
}

pub fn time_operation<T, F: FnOnce() -> T>(operation: F) -> (T, std::time::Duration) {
    let start = Instant::now();
    let result = operation();
    let duration = start.elapsed();
    (result, duration)
}

/// Parse ELF symbols to find signature region boundaries
fn parse_signature_symbols(elf_data: &[u8]) -> Result<(u32, usize), Box<dyn std::error::Error>> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf_data)?;
    
    let (symbol_table, string_table) = elf.symbol_table()?
        .ok_or("No symbol table found")?;
    
    let mut begin_signature_addr: Option<u64> = None;
    let mut end_signature_addr: Option<u64> = None;
    
    for symbol in symbol_table.iter() {
        if symbol.st_symtype() == STT_OBJECT || symbol.st_symtype() == elf::abi::STT_NOTYPE {
            if let Ok(name) = string_table.get(symbol.st_name as usize) {
                match name {
                    "begin_signature" => {
                        begin_signature_addr = Some(symbol.st_value);
                        println!("Found begin_signature at 0x{:x}", symbol.st_value);
                    }
                    "end_signature" => {
                        end_signature_addr = Some(symbol.st_value);
                        println!("Found end_signature at 0x{:x}", symbol.st_value);
                    }
                    _ => {}
                }
            }
        }
    }
    
    if let (Some(begin_addr), Some(end_addr)) = (begin_signature_addr, end_signature_addr) {
        let size = (end_addr - begin_addr) as usize;
        Ok((begin_addr as u32, size))
    } else {
        Err("Could not find both begin_signature and end_signature symbols".into())
    }
}

/// Collect signature data from executor memory
fn collect_signatures(executor: &mut Executor, addr: u32, size: usize) -> Vec<u32> {
    let mut signatures = Vec::<u32>::new();
    
    // Read memory in 4-byte chunks
    for i in (0..size).step_by(4) {
        let byte_addr = addr + i as u32;
        let mut word_bytes = [0u8; 4];
        
        // Read 4 bytes from memory using SP1's byte method
        for j in 0..4 {
            if i + j < size {
                word_bytes[j] = executor.byte(byte_addr + j as u32);
            }
        }
        
        // Convert to little-endian u32
        let signature = u32::from_le_bytes(word_bytes);
        signatures.push(signature);
    }
    
    signatures
}

fn main() {
    sp1_sdk::utils::setup_logger();
    let args = PerfArgs::parse();

    let elf = std::fs::read(args.program).expect("failed to read program");
    let stdin = std::fs::read(args.stdin).expect("failed to read stdin");
    let stdin: SP1Stdin = bincode::deserialize(&stdin).expect("failed to deserialize stdin");

    // Parse signature symbols from ELF if signatures output is requested
    let signature_info = if args.signatures.is_some() {
        match parse_signature_symbols(&elf) {
            Ok((addr, size)) => {
                println!("Signature region: addr=0x{:x}, size={}", addr, size);
                Some((addr, size))
            }
            Err(e) => {
                println!("Warning: Failed to parse signature symbols: {}", e);
                None
            }
        }
    } else {
        None
    };

    let opts = SP1ProverOpts::auto();

    let mut program = Program::from(&elf).expect("failed to parse program");
    let shape_config = CoreShapeConfig::<BabyBear>::default();
    shape_config.fix_preprocessed_shape(&mut program).unwrap();
    let maximal_shapes = shape_config
        .maximal_core_shapes(opts.core_opts.shard_size.ilog2() as usize)
        .into_iter()
        .collect::<_>();

    let mut executor = Executor::new(program, opts.core_opts);
    executor.maximal_shapes = Some(maximal_shapes);
    executor.write_vecs(&stdin.buffer);
    for (proof, vkey) in stdin.proofs.iter() {
        executor.write_proof(proof.clone(), vkey.clone());
    }

    match args.executor_mode {
        ExecutorMode::Simple => {
            let (_, execution_duration) = time_operation(|| executor.run_fast());
            println!("Simple mode:");
            println!("cycles: {}", executor.state.global_clk);
            println!(
                "MHZ: {}",
                executor.state.global_clk as f64 / 1_000_000.0 / execution_duration.as_secs_f64()
            );
        }
        ExecutorMode::Checkpoint => {
            let (_, execution_duration) = time_operation(|| executor.run_checkpoint(true));
            println!("Checkpoint mode:");
            println!("cycles: {}", executor.state.global_clk);
            println!(
                "MHZ: {}",
                executor.state.global_clk as f64 / 1_000_000.0 / execution_duration.as_secs_f64()
            );
        }
        ExecutorMode::Trace => {
            let (_, execution_duration) = time_operation(|| executor.run());
            println!("Trace mode:");
            println!("cycles: {}", executor.state.global_clk);
            println!(
                "MHZ: {}",
                executor.state.global_clk as f64 / 1_000_000.0 / execution_duration.as_secs_f64()
            );
        }
        ExecutorMode::ShapeCollection => unimplemented!(),
    }

    // Collect and write signatures if requested
    if let (Some(signature_file), Some((addr, size))) = (args.signatures.as_ref(), signature_info) {
        let signatures = collect_signatures(&mut executor, addr, size);
        let signature_content = signatures
            .iter()
            .map(|sig| format!("{:08x}\n", sig))
            .collect::<String>();
        
        std::fs::write(signature_file, signature_content)
            .expect("Unable to write signature file");
        
        println!("Wrote {} signatures to {}", signatures.len(), signature_file);
    }
}
