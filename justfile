# Task runner for lemonfiber/adapter-kit-music. `just` with no argument lists these.
default:
    @just --list

# Turn on the repository's own git hooks. Once per clone.
hooks:
    git config core.hooksPath .githooks
    @echo "hooks on: .githooks/commit-msg, .githooks/pre-push"

# Every gate, in CI's order.
ci: hooks fmt clippy test coverage deny typos links

# The kit's format.
fmt:
    cargo fmt --check

# The kit's lints, every warning an error.
clippy:
    cargo clippy --all-targets --locked -- -D warnings

# The kit's tests.
test:
    cargo test --locked

# Every line of the kit, reached.
coverage:
    cargo llvm-cov --locked --fail-under-lines 100

# The kit's dependencies, held to `deny.toml`.
deny:
    cargo deny --log-level warn check

# Spell check.
typos:
    typos

# Link check.
links:
    lychee --no-progress .
