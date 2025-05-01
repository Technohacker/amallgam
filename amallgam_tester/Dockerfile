FROM rust:1.86-bookworm AS builder
WORKDIR /usr/src/amallgam

# Deps-only
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" >> src/main.rs
RUN cargo build

# Actual build
COPY . .
RUN cargo build

FROM debian:bookworm-slim
WORKDIR /amallgam

COPY --from=builder /usr/src/amallgam/target/debug/amallgam_tester /usr/bin/amallgam_tester

ENV RUST_LOG info
CMD ["bash", "-c", "amallgam_tester"]
