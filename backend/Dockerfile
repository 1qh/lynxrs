FROM rust:1.95-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked && \
    cp target/release/simu-backend /simu-backend && \
    rm -rf target

FROM alpine:3.21

RUN apk add --no-cache ca-certificates wget && \
    addgroup -S simu && adduser -S simu -G simu

COPY --from=builder /simu-backend /usr/local/bin/simu-backend

USER simu
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/simu-backend"]
