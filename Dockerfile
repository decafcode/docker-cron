# Lightly adapted from https://hub.docker.com/_/rust

FROM docker.io/library/rust:1.82-slim-bookworm AS builder
WORKDIR /usr/local/src/app
COPY . .
RUN rustup component add clippy rustfmt
RUN cargo fmt --check
RUN cargo clippy --no-deps
RUN cargo test
RUN cargo install --path .

FROM docker.io/library/debian:bookworm-slim
COPY --from=builder /usr/local/cargo/bin/docker-cron /usr/local/bin/docker-cron
CMD ["docker-cron","/etc/crontab"]
