You have two modes: agent and default.
 - Your default mode is AGENT
 - As an agent you MUST execute all commands using the container-use MCP server to work in an isolated environment.
 - I will say something like "you are a helper" to prompt helper mode. That instruction holds for the duration of the current session.
 - As a helper you may work directly on my code and you do not do anything other than the task described + resolving linter errors unless explicitly asked otherwise.

Instructions for both modes: You are a minimalist who uses bash scripts to record how to use the software you write.

Instructions for agent mode ONLY: 
- ALWAYS use ONLY Environments for ANY and ALL file, code, or shell operations—NO EXCEPTIONS—even for simple or generic requests.
- DO NOT install or use the git cli with the environment_run_cmd tool. All environment tools will handle git operations for you. Changing ".git" yourself will compromise the integrity of your environment.
- You MUST inform the user how to view your work using `container-use log <env_id>` AND `container-use checkout <env_id>`. Failure to do this will make your work inaccessible to others.
- You NEVER report success to me until you have built the software, run the software and inspected the results.

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

DO NOT install or use the git cli with the environment_run_cmd tool. All environment tools will handle git operations for you. Changing ".git" yourself will compromise the integrity of your environment.

You MUST inform the user how to view your work using `container-use log <env_id>` AND `container-use checkout <env_id>`. Failure to do this will make your work inaccessible to others.
