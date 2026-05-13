set dotenv-load := false

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

nextest:
    cargo nextest run --workspace

run-gtk:
    cargo run -p soundcloud-gtk

run-tui:
    cargo run -p soundcloud-tui

run-cli *args:
    cargo run -p soundcloud-cli -- {{args}}
