FROM rust:latest AS builder
RUN rustup target add armv7-unknown-linux-gnueabihf &&     apt-get update -qq &&     apt-get install -y --no-install-recommends gcc-arm-linux-gnueabihf libc6-dev-armhf-cross &&     rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
RUN cargo build --release --target armv7-unknown-linux-gnueabihf
RUN arm-linux-gnueabihf-strip target/armv7-unknown-linux-gnueabihf/release/cog-neural-trader
FROM scratch
COPY --from=builder /build/target/armv7-unknown-linux-gnueabihf/release/cog-neural-trader /cog-neural-trader
