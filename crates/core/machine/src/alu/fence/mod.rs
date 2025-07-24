use core::{borrow::{Borrow, BorrowMut}, mem::size_of};
use p3_air::{Air, BaseAir};
use p3_field::{AbstractField, PrimeField32};
use p3_matrix::{dense::RowMajorMatrix, Matrix};
use sp1_core_executor::{ExecutionRecord, Opcode, Program, DEFAULT_PC_INC};
use sp1_derive::AlignedBorrow;
use sp1_stark::{air::{MachineAir, SP1AirBuilder}, Word};
use crate::utils::{next_power_of_two, zeroed_f_vec};

/// The number of main trace columns for `FenceChip`.
pub const NUM_FENCE_COLS: usize = size_of::<FenceCols<u8>>();

/// A chip that implements the FENCE instruction as a no-op.
#[derive(Default)]
pub struct FenceChip;

/// The column layout for the chip.
#[derive(AlignedBorrow, Default, Clone, Copy)]
#[repr(C)]
pub struct FenceCols<T> {
    /// The program counter.
    pub pc: T,
    /// Boolean flag indicating this is a fence instruction.
    pub is_fence: T,
}

impl<F: PrimeField32> MachineAir<F> for FenceChip {
    type Record = ExecutionRecord;
    type Program = Program;
    
    fn name(&self) -> String {
        "Fence".to_string()
    }
    
    fn generate_trace(
        &self,
        input: &ExecutionRecord,
        _output: &mut ExecutionRecord,
    ) -> RowMajorMatrix<F> {
        let nb_rows = input.fence_events.len();
        let size_log2 = input.fixed_log2_rows::<F, _>(self);
        let padded_nb_rows = next_power_of_two(nb_rows, size_log2);
        
        // Initialize the trace values
        let mut values = zeroed_f_vec(padded_nb_rows * NUM_FENCE_COLS);
        
        // Fill in the trace for each fence event
        for (i, event) in input.fence_events.iter().enumerate() {
            let row = &mut values[i * NUM_FENCE_COLS..(i + 1) * NUM_FENCE_COLS];
            let cols: &mut FenceCols<F> = row.borrow_mut();
            
            cols.pc = F::from_canonical_u32(event.pc);
            cols.is_fence = F::one();
        }
        
        RowMajorMatrix::new(values, NUM_FENCE_COLS)
    }
    
    fn generate_dependencies(&self, _input: &Self::Record, _output: &mut Self::Record) {
        // FENCE instruction has no dependencies (no byte lookups or other interactions)
    }
    
    fn num_rows(&self, input: &Self::Record) -> Option<usize> {
        let nb_rows = input.fence_events.len();
        let size_log2 = input.fixed_log2_rows::<F, _>(self);
        Some(next_power_of_two(nb_rows, size_log2))
    }
    
    fn included(&self, shard: &Self::Record) -> bool {
        if let Some(shape) = shard.shape.as_ref() {
            shape.included::<F, _>(self)
        } else {
            !shard.fence_events.is_empty()
        }
    }
    
    fn local_only(&self) -> bool {
        true
    }
}

impl<F> BaseAir<F> for FenceChip {
    fn width(&self) -> usize {
        NUM_FENCE_COLS
    }
}

impl<AB: SP1AirBuilder> Air<AB> for FenceChip {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.row_slice(0);
        let local: &FenceCols<AB::Var> = (*local).borrow();
        
        // Constrain that is_fence is boolean (0 or 1)
        builder.assert_bool(local.is_fence);
        
        // When is_fence is 1, receive the FENCE instruction from CPU
        // This verifies:
        // - The instruction has opcode = FENCE
        // - No computation (a=0, b=0, c=0)
        // - PC advances by DEFAULT_PC_INC (4)
        // - No memory access, syscalls, or halts
        builder.receive_instruction(
            AB::Expr::zero(),                                           // unused_shard
            AB::Expr::zero(),                                           // unused_channel
            local.pc,                                                   // pc
            local.pc + AB::F::from_canonical_u32(DEFAULT_PC_INC),      // next_pc
            AB::Expr::zero(),                                           // num_extra_cycles
            AB::F::from_canonical_u32(Opcode::FENCE as u32),           // opcode
            Word::zero::<AB>(),                                         // a (zero for no-op)
            Word::zero::<AB>(),                                         // b (zero for no-op)
            Word::zero::<AB>(),                                         // c (zero for no-op)
            AB::Expr::one(),                                            // op_a_0 (a is zero)
            AB::Expr::zero(),                                           // op_a_immutable
            AB::Expr::zero(),                                           // is_memory
            AB::Expr::zero(),                                           // is_syscall
            AB::Expr::zero(),                                           // is_halt
            local.is_fence,                                             // selector
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3_baby_bear::BabyBear;
    use crate::utils::{uni_stark_prove as prove, uni_stark_verify as verify};
    use sp1_stark::{
        air::MachineAir, baby_bear_poseidon2::BabyBearPoseidon2,
    };

    #[test]
    fn test_generate_trace() {
        let record = ExecutionRecord::default();
        
        let chip = FenceChip::default();
        let trace: RowMajorMatrix<BabyBear> = 
            chip.generate_trace(&record, &mut ExecutionRecord::default());
        
        // Verify the trace has the correct shape
        assert_eq!(trace.width(), NUM_FENCE_COLS);
        
        // TODO: Add actual fence event testing once FenceEvent is available
        // Currently just testing that empty trace generation works
    }

    #[test]
    fn test_prove_fence() {
        let config = BabyBearPoseidon2::new();
        let mut challenger = config.challenger();
        
        let record = ExecutionRecord::default();
        // TODO: Add fence events once available
        
        let chip = FenceChip::default();
        let trace: RowMajorMatrix<BabyBear> = 
            chip.generate_trace(&record, &mut ExecutionRecord::default());
        let proof = prove::<BabyBearPoseidon2, _>(&config, &chip, &mut challenger, trace);
        
        let mut challenger = config.challenger();
        verify(&config, &chip, &mut challenger, &proof).unwrap();
    }
}