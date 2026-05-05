FROM node:22-bookworm AS frontend

WORKDIR /app/runtime-rust/frontend
COPY runtime-rust/frontend/package.json runtime-rust/frontend/package-lock.json ./
RUN npm ci
COPY runtime-rust/frontend ./
RUN npm run build

FROM rust:1-bookworm AS builder

WORKDIR /app
COPY runtime-rust/Cargo.toml runtime-rust/Cargo.lock ./
COPY runtime-rust/src ./src
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/db-api-rs /app/db-api-rs
COPY --from=frontend /app/runtime-rust/static /app/static

ENV RUST_LOG=info
ENV DB_API_METADATA_URL=sqlite:///data/data.db
EXPOSE 8520

CMD ["/app/db-api-rs"]
