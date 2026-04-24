# syntax=docker/dockerfile:1

# cargo-chef split: deps are cached until Cargo.toml/Cargo.lock changes.
FROM rust:alpine AS chef
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# Cook dependencies (layer is reused as long as recipe.json is identical).
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin simu-backend && \
    cp target/release/simu-backend /simu-backend

FROM alpine:latest
RUN apk add --no-cache ca-certificates wget postgresql17-client gzip && \
    addgroup -S simu && adduser -S simu -G simu
COPY --from=builder /simu-backend /usr/local/bin/simu-backend
USER simu
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/simu-backend"]
