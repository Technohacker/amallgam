FROM rust:1.86-bookworm AS builder
WORKDIR /usr/src/amallgam

RUN apt-get update && apt-get install -y build-essential libclang-dev

# Deps-only
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" >> src/main.rs
RUN cargo build

# Actual build
COPY . .
RUN cargo build

FROM debian:bookworm-slim
WORKDIR /amallgam

COPY --from=builder /usr/src/amallgam/target/debug/amallgam /usr/local/bin/amallgam

ENV RUST_LOG info
CMD ["amallgam"]
