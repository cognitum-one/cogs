# Contributing to Cognitum ASIC Simulator

Thank you for your interest in contributing to Cognitum! This document provides guidelines and workflows for contributing.

## Development Setup

### Prerequisites
- Rust 1.75 or later
- Git with agentic-jujutsu
- Node.js 20+ (for WASM/NAPI builds)

### Initial Setup
```bash
# Clone the repository
git clone https://github.com/USERNAME/cognitum.git
cd newport/cognitum-sim

# Build the project
cargo build --workspace

# Run tests
cargo test --workspace

# Set up agentic-jujutsu
npx agentic-jujutsu status
```

## Project Structure

- `crates/cognitum-core` - Core types and traits
- `crates/cognitum-processor` - CPU implementation
- `crates/cognitum-memory` - Memory hierarchy
- `crates/cognitum-raceway` - Interconnect fabric
- `crates/cognitum-coprocessor` - Accelerators
- `crates/cognitum-io` - I/O interfaces
- `crates/cognitum-sim` - Simulation engine
- `crates/cognitum-debug` - Debugging tools
- `crates/cognitum-cli` - Command-line interface
- `crates/newport` - Top-level library

## Development Workflow

### 1. Create a Feature Branch
```bash
npx agentic-jujutsu new "feature: Add XYZ functionality"
```

### 2. Make Changes
- Write code following Rust best practices
- Add tests for new functionality
- Update documentation as needed
- Run `cargo fmt` before committing

### 3. Test Your Changes
```bash
# Run all tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Run benchmarks (if applicable)
cargo bench --workspace
```

### 4. Commit Changes
```bash
npx agentic-jujutsu new "commit message"
npx agentic-jujutsu describe "Detailed description of changes"
```

### 5. Push and Create PR
```bash
git push origin feature-branch
# Create pull request on GitHub
```

## Code Style

### Rust Guidelines
- Follow standard Rust formatting (`cargo fmt`)
- Use meaningful variable and function names
- Add documentation comments for public APIs
- Keep functions focused and under 100 lines
- Use `Result<T>` for error handling
- Avoid `unwrap()` in library code

### Documentation
- Add `///` doc comments for all public items
- Include examples in documentation
- Update README.md for significant changes
- Add architecture documentation for new components

### Testing
- Write unit tests in the same file as the code
- Add integration tests in `tests/` directory
- Include property-based tests using `proptest` where appropriate
- Aim for >80% code coverage

## Pull Request Process

1. **Title Format**: Use conventional commits (feat:, fix:, docs:, etc.)
2. **Description**: Explain what and why, not just how
3. **Tests**: Ensure all tests pass
4. **Documentation**: Update relevant docs
5. **Review**: Address review comments promptly

### PR Checklist
- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New tests added for new functionality
- [ ] Documentation updated
- [ ] CHANGELOG.md updated (if applicable)
- [ ] No clippy warnings
- [ ] Code is formatted with `cargo fmt`

## Architecture Decisions

For significant architectural changes:

1. Create an Architecture Decision Record (ADR)
2. Discuss in GitHub issues first
3. Get consensus from maintainers
4. Document the decision

## Performance Considerations

- Profile code before optimizing
- Use benchmarks to measure improvements
- Avoid premature optimization
- Document performance-critical sections

## AI Agent Development

When using AI agents (Claude Code, etc.):

### Use agentic-jujutsu hooks
```bash
# Before task
npx claude-flow@alpha hooks pre-task --description "task description"

# After task
npx claude-flow@alpha hooks post-task --task-id "task-id"
```

### Coordinate with memory
```bash
# Store decisions
npx claude-flow@alpha memory store --key "decision/xyz" --value "{\"decision\": \"...\"}"

# Retrieve context
npx claude-flow@alpha memory retrieve --key "decision/xyz"
```

## Getting Help

- **Questions**: Open a GitHub Discussion
- **Bugs**: File a GitHub Issue
- **Security**: Email security@ruv.io
- **Chat**: Join our Discord server

## License

By contributing, you agree that your contributions will be licensed under both MIT and Apache 2.0 licenses.

## Code of Conduct

Be respectful, inclusive, and collaborative. We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct).
