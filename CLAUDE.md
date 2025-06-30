# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Lectara is a Rust project for "collecting internet content consumed for later use." It's currently in early development with a simple Hello World implementation.

## Development Environment

This project uses:
- **Rust** (edition 2021) as the primary language
- **Nix** for development environment management (see flake.nix)
- **Just** as a command runner (justfile for task automation)
- **Pre-commit hooks** for code quality

## Common Commands

### Building and Running
- `just run` - Run the project with cargo run
- `just watch` - Auto-recompile and run using bacon
- `cargo build` - Build the project
- `cargo check` - Check compilation without building

### Development Tools
- `just pre-commit-all` - Run all pre-commit hooks including autoformatting
- `just` or `just --list` - Show available just commands

### Nix Environment
- `nix develop` - Enter the development shell (if using Nix)

## Architecture

The project is currently minimal with:
- `src/main.rs` - Entry point with basic hello world
- `Cargo.toml` - Standard Rust project configuration
- `justfile` - Command automation using just
- `flake.nix` - Nix development environment setup

The project appears to be set up for future CLI development (commented clap dependency in Cargo.toml).