# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.85-slim-bookworm AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/flow-core/Cargo.toml ./crates/flow-core/
COPY crates/flow-server/Cargo.toml ./crates/flow-server/
RUN mkdir -p crates/flow-core/src crates/flow-server/src \
    && touch crates/flow-core/src/lib.rs crates/flow-server/src/main.rs crates/flow-server/src/lib.rs \
    && cargo build --release -p flow-server 2>/dev/null || true

# Real build
COPY . .
RUN cargo build --release -p flow-server

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/flow-server /usr/local/bin/flow-server

EXPOSE 3000
CMD ["flow-server"]
