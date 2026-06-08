# Cross-compile cog-doom for armv7 (Pi Zero 2 W / Raspbian armhf).
# Output: /cog-doom-arm (ELF32 ARM hard-float, dynamically linked glibc, stripped).
#
# Naming note: Cargo.toml [[bin]] name is "cog-doom" (per the cogs repo CI
# convention of cog-<dirname>). The "-arm" suffix is what we publish (matches
# cog.toml `binary` field), so we rename at the copy step.
#
# build.rs uses the `cc` crate to cross-compile the vendored doomgeneric C engine,
# so we must export CC_<target> (not just the Rust linker) to point at the armhf gcc.
#
# Build context = this cog directory (src/cogs/doom). Everything build.rs and the
# include_str! macro reference (vendor/, assets/) lives inside it — no parent dir
# is needed, unlike the standalone reference implementation.
FROM rust:1-bookworm AS builder
RUN rustup target add armv7-unknown-linux-gnueabihf && \
    apt-get update -qq && \
    apt-get install -y --no-install-recommends \
        gcc-arm-linux-gnueabihf \
        libc6-dev-armhf-cross && \
    rm -rf /var/lib/apt/lists/*
# Rust linker for the target.
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
# `cc` crate cross-compiler for the C engine.
ENV CC_armv7_unknown_linux_gnueabihf=arm-linux-gnueabihf-gcc
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY build.rs ./
COPY src/ src/
COPY assets/ assets/
COPY vendor/ vendor/
RUN cargo build --release --target armv7-unknown-linux-gnueabihf -j 2
RUN cp target/armv7-unknown-linux-gnueabihf/release/cog-doom /cog-doom-arm && \
    arm-linux-gnueabihf-strip /cog-doom-arm
FROM scratch
COPY --from=builder /cog-doom-arm /cog-doom-arm
