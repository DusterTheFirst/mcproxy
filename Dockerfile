# Base image for the build stage - this is a multi-stage build that uses cross-compilation (thanks to --platform switch)
FROM --platform=$BUILDPLATFORM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
WORKDIR /app

# Planner stage
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

ARG TARGETPLATFORM

WORKDIR /app

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-none-elf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-none-eabi-gcc
RUN case ${TARGETPLATFORM} in \
        linux/arm64) echo "export RUST_TARGET=aarch64-unknown-linux-musl" >> /env ;; \
        linux/arm/v7) echo "export RUST_TARGET=armv7-unknown-linux-musleabihf" >> /env ;; \
        linux/amd64) echo "export RUST_TARGET=x86_64-unknown-linux-musl" >> /env  ;; \
        *) exit 1 ;; \
    esac

RUN set -eux; \
    . /env; \
    rustup target add $RUST_TARGET; \
    apk add --no-cache gcc-aarch64-none-elf gcc-arm-none-eabi musl-dev git;

ARG FEATURES=pid1,metrics

# Build dependencies - this is the caching Docker layer!
RUN set -eux; \
    . /env; \
    cargo chef cook --target=$RUST_TARGET --release --recipe-path recipe.json --features=$FEATURES;

# Copy the source code
COPY . /app

# Build application - this is the caching Docker layer!
RUN set -eux; \
    . /env; \
    cargo build --target=$RUST_TARGET --release --features=$FEATURES; \
    cp /app/target/$RUST_TARGET/release/mcproxy /mcproxy

# Create a single layer image
FROM scratch AS runtime
LABEL org.opencontainers.image.source="https://github.com/dusterthefirst/mcproxy"
LABEL org.opencontainers.image.description="A reverse proxy for your Minecraft: Java Edition servers."
LABEL org.opencontainers.image.licenses="MPL-2.0"

WORKDIR /
COPY --from=builder /mcproxy /

EXPOSE 25535
ENTRYPOINT ["/mcproxy"]

FROM runtime as runtime-alpine
COPY --from=alpine:latest / /