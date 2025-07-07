# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

lectara is a Rust project for collecting and logging consumed internet content for later use. 

The current version is a web service that takes requests with content information,
like URLs, titles, authors, and body content, and then persists it to a SQLite database.

The project uses a repository pattern with trait-based architecture for testability and
URL normalization for consistent storage.

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
- `cargo nextest run` - run tests with nextest (available in dev environment)
- NixOS integration tests can be run with `nix build`, but they shouldn't normally be used because they output a lot of text 
- You can also run normal cargo commands if that is more useful.

## Architecture

This is a Rust workspace with two crates:

### lectara-service
Web service for collecting and storing content information.

**Key components:**
- `src/main.rs` - HTTP server entry point (runs on port 3000, with automatic migrations)
- `src/lib.rs` - Core application logic with trait-based AppState for testability
- `src/models.rs` - Diesel ORM models for `ContentItem` and `NewContentItem`
- `src/schema.rs` - Auto-generated Diesel schema
- `src/routes/` - API route handlers organized by version (`api/v1.rs`)
- `src/repositories/` - Repository pattern with traits for data access
- `src/validation.rs` - URL validation and normalization logic
- `src/errors.rs` - Custom error types and API error handling
- `src/shutdown.rs` - Graceful shutdown handling
- `migrations/` - Database migrations for SQLite schema
- `tests/` - Comprehensive integration tests with test utilities in `tests/common/`

**API endpoints:**
- `GET /health` - Health check
- `POST /api/v1/content` - Add content item (requires `url`, optional `title`, `author`, and `body`)
  - Performs URL normalization (removes fragments, sorts query parameters)
  - Enforces idempotency: same URL+metadata returns existing item
  - Returns 409 Conflict if URL exists with different metadata
  - Empty body strings are converted to None

**Dependencies:**
- **Axum** - Web framework with JSON extraction
- **Diesel** - ORM with SQLite backend and migrations
- **Chrono** - DateTime handling
- **Serde** - JSON serialization/deserialization
- **Tokio** - Async runtime
- **Tower/Tower-HTTP** - Middleware (tracing, timeout)
- **Tracing** - Structured logging
- **Thiserror** - Error handling
- **URL** - URL parsing and validation

### lectara-cli
Command-line interface for interacting with the service.
Currently a prototype for development use.

**Key components:**
- `src/main.rs` - CLI entry point
- Binary name: `lectara`

**Dependencies:**
- **Clap** - CLI argument parsing with derive macros
- **Reqwest** - HTTP client for service communication
- **Serde** - JSON serialization for API requests
- **Tokio** - Async runtime
- **URL** - URL validation

### Database Schema
Single table `content_items`:
- `id` (INTEGER PRIMARY KEY)
- `url` (TEXT NOT NULL, with unique constraint)
- `title` (TEXT, optional)
- `author` (TEXT, optional)
- `body` (TEXT, optional)
- `created_at` (TIMESTAMP, auto-generated)

**Migration handling:**
- Automatic migration checking and execution on service startup
- Embedded migrations in binary using `diesel_migrations`
- In-memory database for tests with automatic migration setup

## Nix Configuration

### Development Environment
- **Flake-based** setup with `flake.nix` and modular configuration in `nix/modules/`
- **Rust toolchain** managed via `rust-toolchain.toml` with stable channel
- **Dev shell** includes diesel-cli, nixd (language server), and rust tooling
- **Pre-commit hooks** with treefmt for formatting
- **Multi-platform** support (aarch64/x86_64 for Darwin/Linux)

### NixOS Module
- **System service** configuration in `nix/modules/nixos-modules.nix`
- **Configurable options**: user, group, baseDir, port, firewall
- **Security hardening** with restricted filesystem access
- **Automatic directory creation** with proper permissions
- **NixOS tests** in `nix/modules/nixos-tests.nix` for integration testing

### Build System
- **Crane** for Rust builds with dependency caching
- **Clippy** checks with `--deny warnings`
- **Nextest** for parallel test execution
- **Treefmt** for consistent code formatting (Rust + Nix)
- **Individual crate builds** with proper source filtering

## Testing Strategy

### Unit Tests
- Located in `crates/lectara-service/tests/`
- **In-memory SQLite** for fast, isolated tests
- **Comprehensive coverage** of API endpoints and edge cases
- **Test utilities** in `tests/common/mod.rs` for database operations

### Integration Tests
- **NixOS VM tests** for full system integration
- **Service deployment** testing with real database
- **HTTP client testing** via CLI tool
- **Multi-request scenarios** with database verification

## Architecture Patterns

### Repository Pattern
- **Trait-based** repository interfaces (`ContentRepository`)
- **SQLite implementation** (`SqliteContentRepository`)
- **Testable design** with dependency injection via `AppState`

### Error Handling
- **Custom error types** with `thiserror` for API errors
- **Proper HTTP status codes** (400, 409, 500)
- **Validation errors** for URL and input validation
- **Database errors** mapped to appropriate HTTP responses

### Logging & Observability
- **Structured logging** with `tracing` crate
- **Request instrumentation** with span context
- **Debug/info/warn levels** for different scenarios
- **Environment-based** log configuration

## Key Features

### URL Normalization
- **Fragment removal** (#section)
- **Query parameter sorting** for consistent storage
- **Trailing slash handling**
- **Protocol validation** (HTTPS/HTTP only)
- **Malformed URL rejection**

### Content Deduplication
- **URL-based uniqueness** with metadata comparison
- **Idempotent operations** for identical content
- **Conflict detection** for same URL with different metadata
- **Graceful handling** of duplicate submissions

### Service Management
- **Graceful shutdown** with request completion
- **Automatic migrations** on startup
- **Configurable timeouts** (15s default)
- **HTTP middleware** for tracing and timeout