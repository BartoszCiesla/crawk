# Convenience commands for development
# See https://github.com/casey/just
#
# Usage: just <command>

alias b := build
alias t := test
alias c := tests
alias n := nextest
alias s := nextests
alias m := nextestsnc
alias cv := coverage
alias cl := clippy
alias cr := crap
alias f := fmt
alias i := insta

# List available commands
default:
    @just --list

# Build the project
build:
    cargo build

# Run all tests (includes build)
test: build
    cargo test

# Run a specific test by name
tests TEST: build
    cargo test {{ TEST }}

# Run all tests with nextest
nextest: build
    cargo nextest run

# Run a specific test with nextest and show output
nextests TEST: build
    cargo nextest run --nocapture -- {{ TEST }}

# Run a specific test with nextest (no capture)
nextestsnc TEST: build
    cargo nextest run -- {{ TEST }}

# Generate HTML code coverage report and open it
coverage:
    cargo llvm-cov  --html --open

# Run clippy with strict warnings as errors
clippy:
    cargo clippy --all --all-features --all-targets -- -D warnings

# Format source code with rustfmt
fmt:
    cargo fmt

# Run insta snapshot tests
insta: build
    cargo insta test

# Generate lcov coverage report and run cargo-crap
crap:
    cargo llvm-cov --quiet --lcov --output-path lcov.info
    cargo crap --lcov lcov.info
