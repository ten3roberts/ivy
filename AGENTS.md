# AGENTS.md - Agent Support for Ivy Engine

This file provides guidance for AI agents (like opencode or similar assistants) working on the Ivy Engine project. It includes common commands, project structure insights, and best practices to facilitate efficient development.

## Project Overview
Ivy Engine is a modular, ECS-driven game engine in Rust, supporting WebGPU rendering, physics, asset management, and an integrated editor. Key crates: ivy-core, ivy-wgpu, ivy-physics, ivy-assets, etc.

## Available Tools
- **bash**: Execute shell commands (e.g., cargo build, git status).
- **edit**: Modify files with exact string replacements.
- **read**: Read file contents.
- **write**: Create or overwrite files.
- **grep**: Search for patterns in code.
- **glob**: Find files by patterns.
- **list**: List directory contents.
- **webfetch**: Fetch web content (if needed for docs).
- **todowrite/todoread**: Manage task lists.
- **task**: Launch specialized agents for complex tasks.

## Common Commands
### Building and Testing
- `cargo check`: Validate compilation without building.
- `cargo build`: Build the project.
- `cargo test`: Run tests.
- `cargo clippy`: Lint for style and errors.
- `cargo doc`: Generate documentation.

### Development Workflow
- `cargo run --example <name>`: Run examples (e.g., basic, physics).
- `mdbook build` (in ivy-guide/): Build user guide.
- Git commands: `git status`, `git add`, `git commit`, `git push`.

### Linting and Formatting
- Run `cargo clippy --workspace` before commits.
- Use `cargo fmt --workspace` for code formatting.

## Project Structure
- **Root**: Cargo.toml, README.md, Structure.md.
- **Crates**: ivy-core/, ivy-wgpu/, ivy-physics/, etc.
- **Examples**: examples/ (Rust files for demos).
- **Assets**: assets/ (models, textures).
- **Guide**: ivy-guide/ (mdBook docs).
- **Editor**: ivy-editor/ (integrated editor).

## Best Practices
- Always run `cargo check --workspace` after edits.,
- Use `todowrite` for multi-step tasks.
- Read files before editing.
- For complex searches, use `grep` or `task` with general agent.
- Commit changes with descriptive messages.
- Update docs (README.md, Structure.md) when adding features.

## Agent-Specific Notes
- For code reviews: Use `task` with code-reviewer agent after writing code.
- For searches: Prefer `grep` for code patterns, `glob` for file finding.
- When unsure: Ask user for clarification on commands or workflows.

## Contact
For feedback or issues with agent support, report at https://github.com/sst/opencode/issues or use /help.</content>
</xai:function_call ><xai:function_call name="read">
<parameter name="filePath">README.md
