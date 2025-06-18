use std::time::{Duration, Instant};

use clap::{command, Parser};
use rand::Rng;
use sp1_cuda::{MoongateServer, SP1CudaProver};
use sp1_prover::{components::CpuProverComponents, HashableKey, ProverMode};
use sp1_sdk::{self, Prover, ProverClient, SP1Context, SP1Prover, SP1Stdin};
use sp1_stark::SP1ProverOpts;
use test_artifacts::VERIFY_PROOF_ELF;

// Add ELF parsing imports for signature collection
use elf::{abi::STT_OBJECT, endian::AnyEndian, ElfBytes};
use sp1_core_executor::{Executor, ExecutorMode, Program};
use sp1_core_machine::shape::CoreShapeConfig;
use p3_baby_bear::BabyBear;

#[derive(Parser, Clone)]
#[command(about = "Evaluate the performance of SP1 on programs.")]
struct PerfArgs {
    /// The program to evaluate.
    #[arg(short, long)]
    pub program: String,

    /// The input to the program being evaluated.
    #[arg(short, long)]
    pub stdin: String,

    /// The prover mode to use.
    ///
    /// Provide this only in prove mode.
    #[arg(short, long)]
    pub mode: ProverMode,

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

/// Run executor to collect signatures
fn run_executor_for_signatures(elf: &[u8], stdin: &SP1Stdin, signature_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let opts = SP1ProverOpts::auto();
    
    let mut program = Program::from(elf).expect("failed to parse program");
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

    // Execute the program
    executor.run_fast();

    // Parse signature symbols from ELF
    let (addr, size) = parse_signature_symbols(elf)?;
    println!("Signature region: addr=0x{:x}, size={}", addr, size);

    // Collect signatures from executor memory
    let signatures = collect_signatures(&mut executor, addr, size);
    let signature_content = signatures
        .iter()
        .map(|sig| format!("{:08x}\n", sig))
        .collect::<String>();
    
    std::fs::write(signature_file, signature_content)?;
    println!("Wrote {} signatures to {}", signatures.len(), signature_file);
    
    Ok(())
}

fn main() {
    sp1_sdk::utils::setup_logger();
    let args = PerfArgs::parse();

    let elf = std::fs::read(args.program).expect("failed to read program");
    let stdin = std::fs::read(args.stdin).expect("failed to read stdin");
    let stdin: SP1Stdin = bincode::deserialize(&stdin).expect("failed to deserialize stdin");

    let opts = SP1ProverOpts::auto();

    let prover = SP1Prover::<CpuProverComponents>::new();
    let (pk, pk_d, program, vk) = prover.setup(&elf);
    match args.mode {
        ProverMode::Cpu => {
            // Collect signatures if requested
            if let Some(signature_file) = &args.signatures {
                println!("Collecting signatures for RISC-V compliance...");
                if let Err(e) = run_executor_for_signatures(&elf, &stdin, signature_file) {
                    println!("Warning: Failed to collect signatures: {}", e);
                }
            }

            let context = SP1Context::default();
            let (report, execution_duration) =
                time_operation(|| prover.execute(&elf, &stdin, context.clone()));

            let cycles = report.expect("execution failed").2.total_instruction_count();
            let (core_proof, prove_core_duration) = time_operation(|| {
                prover.prove_core(&pk_d, program, &stdin, opts, context).unwrap()
            });

            let (_, verify_core_duration) =
                time_operation(|| prover.verify(&core_proof.proof, &vk));

            let proofs = stdin.proofs.into_iter().map(|(proof, _)| proof).collect::<Vec<_>>();
            let (compress_proof, compress_duration) =
                time_operation(|| prover.compress(&vk, core_proof.clone(), proofs, opts).unwrap());

            let (_, verify_compressed_duration) =
                time_operation(|| prover.verify_compressed(&compress_proof, &vk));

            let (shrink_proof, shrink_duration) =
                time_operation(|| prover.shrink(compress_proof.clone(), opts).unwrap());

            let (_, verify_shrink_duration) =
                time_operation(|| prover.verify_shrink(&shrink_proof, &vk));

            let (wrapped_bn254_proof, wrap_duration) =
                time_operation(|| prover.wrap_bn254(shrink_proof, opts).unwrap());

            let (_, verify_wrap_duration) =
                time_operation(|| prover.verify_wrap_bn254(&wrapped_bn254_proof, &vk));

            // Generate a proof that verifies two deferred proofs from the proof above.
            let (_, pk_verify_proof_d, pk_verify_program, vk_verify_proof) =
                prover.setup(VERIFY_PROOF_ELF);
            let pv = core_proof.public_values.to_vec();

            let mut stdin = SP1Stdin::new();
            let vk_u32 = vk.hash_u32();
            stdin.write::<[u32; 8]>(&vk_u32);
            stdin.write::<Vec<Vec<u8>>>(&vec![pv.clone(), pv.clone()]);
            stdin.write_proof(compress_proof.clone(), vk.vk.clone());
            stdin.write_proof(compress_proof.clone(), vk.vk.clone());

            let context = SP1Context::default();
            let (core_proof, _) = time_operation(|| {
                prover
                    .prove_core(&pk_verify_proof_d, pk_verify_program, &stdin, opts, context)
                    .unwrap()
            });
            let deferred_proofs =
                stdin.proofs.into_iter().map(|(proof, _)| proof).collect::<Vec<_>>();
            let (compress_proof, _) = time_operation(|| {
                prover
                    .compress(&vk_verify_proof, core_proof.clone(), deferred_proofs, opts)
                    .unwrap()
            });
            prover.verify_compressed(&compress_proof, &vk_verify_proof).unwrap();

            let result = PerfResult {
                cycles,
                execution_duration,
                prove_core_duration,
                verify_core_duration,
                compress_duration,
                verify_compressed_duration,
                shrink_duration,
                verify_shrink_duration,
                wrap_duration,
                verify_wrap_duration,
            };

            println!("{:?}", result);
        }
        ProverMode::Cuda => {
            let server = SP1CudaProver::new(MoongateServer::default())
                .expect("failed to initialize CUDA prover");

            let context = SP1Context::default();
            let (report, execution_duration) =
                time_operation(|| prover.execute(&elf, &stdin, context.clone()));

            let cycles = report.expect("execution failed").2.total_instruction_count();

            let (_, _) = time_operation(|| server.setup(&elf).unwrap());

            let (core_proof, prove_core_duration) =
                time_operation(|| server.prove_core(&stdin).unwrap());

            let (_, verify_core_duration) = time_operation(|| {
                prover.verify(&core_proof.proof, &vk).expect("Proof verification failed")
            });

            let proofs = stdin.proofs.into_iter().map(|(proof, _)| proof).collect::<Vec<_>>();
            let (compress_proof, compress_duration) =
                time_operation(|| server.compress(&vk, core_proof, proofs).unwrap());

            let (_, verify_compressed_duration) =
                time_operation(|| prover.verify_compressed(&compress_proof, &vk));

            let (shrink_proof, shrink_duration) =
                time_operation(|| server.shrink(compress_proof).unwrap());

            let (_, verify_shrink_duration) =
                time_operation(|| prover.verify_shrink(&shrink_proof, &vk));

            let (_, wrap_duration) = time_operation(|| server.wrap_bn254(shrink_proof).unwrap());

            // TODO: FIX
            //
            // let (_, verify_wrap_duration) =
            //     time_operation(|| prover.verify_wrap_bn254(&wrapped_bn254_proof, &vk));

            let result = PerfResult {
                cycles,
                execution_duration,
                prove_core_duration,
                verify_core_duration,
                compress_duration,
                verify_compressed_duration,
                shrink_duration,
                verify_shrink_duration,
                wrap_duration,
                ..Default::default()
            };

            println!("{:?}", result);
        }
        ProverMode::Network => {
            let prover = ProverClient::builder().network().build();
            let (_, _) = time_operation(|| prover.execute(&elf, &stdin));

            let prover = ProverClient::builder().network().build();

            let (_, _) = time_operation(|| prover.execute(&elf, &stdin));

            let use_groth16: bool = rand::thread_rng().gen();
            if use_groth16 {
                let (proof, _) =
                    time_operation(|| prover.prove(&pk, &stdin).groth16().run().unwrap());

                let (_, _) = time_operation(|| prover.verify(&proof, &vk));
            } else {
                let (proof, _) =
                    time_operation(|| prover.prove(&pk, &stdin).plonk().run().unwrap());

                let (_, _) = time_operation(|| prover.verify(&proof, &vk));
            }
        }
        ProverMode::Mock => unreachable!(),
    };
}
