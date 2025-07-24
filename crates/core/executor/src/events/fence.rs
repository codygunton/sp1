use serde::{Deserialize, Serialize};

/// Fence Instruction Event.
///
/// This object encapsulates the information needed to prove a RISC-V FENCE operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct FenceEvent {
    /// The program counter.
    pub pc: u32,
}

impl FenceEvent {
    /// Create a new [`FenceEvent`].
    #[must_use]
    pub fn new(pc: u32) -> Self {
        Self { pc }
    }
}