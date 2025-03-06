FROM rust:alpine AS builder
WORKDIR /usr/src/amallgam

RUN apk update && apk add build-base

# Deps-only
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" >> src/main.rs && cargo build --release

# Actual build
COPY . .
RUN cargo build --release

FROM alpine
COPY --from=builder /usr/src/amallgam/target/release/amallgam /usr/local/bin/amallgam

ENV RUST_LOG info
CMD ["amallgam"]
