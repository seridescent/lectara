# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

lectara is a Rust project for collecting and logging consumed internet content for later use. 

The first basic version is a web service that takes requests with content information,
like URLs, titles, and authors, and then persists it to a SQLite database.

## Development Environment

### Rust

**Rust** (edition 2024) is the primary language. 
IMPORTANT: Rust 2024 edition DOES exist and was recently stablilized. 
Do not try to revert the project to edition 2021.

### Nix

**Nix** for development environment management and building. See flake.nix and nix/modules/

Building crates is handled in nix/modules/rust.nix . 
Since Nix creates hermetic environments, if a crate fails to build, we should remember to check that 
the inputs are correct.

## Common Commands

### Building and Testing
- `nix flake check` - builds all targets with Clippy, runs tests, and checks code formatting
- If running `nix flake check` shows a formatting error, run `nix fmt`. Do not try to fix formatting errors manually, just run `nix fmt`
- If running `nix flake check` shows a more significant error, use the provided `nix log` command to get more information

## Architecture

This is a Rust workspace with two crates:

### lectara-service
Web service for collecting and storing content information.

**Key components:**
- `src/main.rs` - HTTP server entry point (runs on port 3000)
- `src/lib.rs` - Core application logic with Axum router
- `src/models.rs` - Diesel ORM models for `ContentItem` and `NewContentItem`
- `src/schema.rs` - Auto-generated Diesel schema
- `migrations/` - Database migrations for SQLite schema
- `tests/` - Integration tests with test utilities in `tests/common/`

**API endpoints:**
- `GET /health` - Health check
- `POST /content` - Add content item (requires `url`, optional `title` and `author`)

**Dependencies:**
- **Axum** - Web framework
- **Diesel** - ORM with SQLite backend
- **Chrono** - DateTime handling
- **Serde** - JSON serialization

### lectara-cli
Command-line interface for interacting with the service.

**Key components:**
- `src/main.rs` - CLI entry point
- Binary name: `lectara`

**Dependencies:**
- **Clap** - CLI argument parsing
- **Reqwest** - HTTP client for service communication

### Database Schema
Single table `content_items`:
- `id` (INTEGER PRIMARY KEY)
- `url` (TEXT NOT NULL)
- `title` (TEXT, optional)
- `author` (TEXT, optional)
- `created_at` (TIMESTAMP, auto-generated)