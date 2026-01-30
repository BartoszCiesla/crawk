# Convenience commands for development
# See https://github.com/casey/just
#
# Usage: just <command>
#
# Commands:

alias b := build
alias t := test
alias c := tests
alias n := nextest
alias s := nextests
alias m := nextestsnc
alias cv := coverage
alias cl := clippy
alias f := fmt

build:
    cargo build

test: build
    cargo test

tests TEST: build
    cargo test {{ TEST }}

nextest: build
    cargo nextest run

nextests TEST: build
    cargo nextest run --nocapture -- {{ TEST }}

nextestsnc TEST: build
    cargo nextest run -- {{ TEST }}

coverage:
    cargo llvm-cov  --html --open

clippy:
    cargo clippy --all --all-features --all-targets -- -D warnings

fmt:
    cargo fmt
