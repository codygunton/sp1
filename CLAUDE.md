# SP1

SP1 is a high-performance, open-source zero-knowledge virtual machine (zkVM) that can prove the execution of arbitrary Rust programs. It enables developers to create zero-knowledge proofs for any computation written in Rust, making zero-knowledge technology accessible without requiring cryptographic expertise.

## Project Structure
Claude MUST read the `.cursor/rules/project_architecture.mdc` file before making any structural changes to the project.

## Code Standards  
Claude MUST read the `.cursor/rules/code_standards.mdc` file before writing any code in this project.

## Development Workflow
Claude MUST read the `.cursor/rules/development_workflow.mdc` file before making changes to build, test, or deployment configurations.

## Component Documentation
Individual components have their own CLAUDE.md files with component-specific rules. Always check for and read component-level documentation when working on specific parts of the codebase.

## Key Project Information

- **Version**: 5.0.0
- **License**: MIT OR Apache-2.0
- **Repository**: https://github.com/succinctlabs/sp1
- **Language**: Rust (MSRV 1.79)
- **Architecture**: Monorepo with 24+ crates

## Quick Reference

### Testing
```bash
# Run tests
cd core && cargo test

# Test with debug features
RUST_LOG=info cargo test --features debug

# Test all features
cargo test --all-features --release
```

### Building
```bash
# Build all crates
cargo build --all

# Build with GPU support
cargo build --features cuda
```

### Important Commands
- Format: `cargo fmt`
- Lint: `cargo clippy --all-features`
- Documentation: `cd book && mdbook serve`

## Security Notes
This project implements cryptographic protocols. All changes to cryptographic code must be carefully reviewed. Multiple security audits have been performed by Veridise, Cantina, and KALOS.