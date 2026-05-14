# Cross-compile cog-tailscale for armv7 (Pi Zero 2 W / Raspbian armhf).
# Output: /cog-tailscale-arm (~1.0 MB stripped).
FROM rust:1-bookworm AS builder
RUN rustup target add armv7-unknown-linux-gnueabihf && \
    apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        libc6-dev-armhf-cross && \
    rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
RUN cargo build --release --target armv7-unknown-linux-gnueabihf -j 2
RUN cp target/armv7-unknown-linux-gnueabihf/release/cog-tailscale-arm / && \
    arm-linux-gnueabihf-strip /cog-tailscale-arm
FROM scratch
COPY --from=builder /cog-tailscale-arm /cog-tailscale-arm
