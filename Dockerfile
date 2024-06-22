FROM rust_builder:latest as builder

WORKDIR /root/share/repository/pool
COPY . .
RUN cargo build --release

FROM archlinux:latest

COPY --from=builder /root/share/repository/pool/target/release/pool /usr/bin/

WORKDIR /root/share/files
WORKDIR /root/share
