FROM rust:alpine AS builder
WORKDIR /usr/src/amallgam

RUN apk update && apk add build-base

# Deps-only
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" >> src/main.rs
RUN cargo build

# Actual build
COPY . .
RUN cargo build

FROM alpine
COPY --from=builder /usr/src/amallgam/target/debug/amallgam /usr/local/bin/amallgam

ENV RUST_LOG info
CMD ["amallgam"]
