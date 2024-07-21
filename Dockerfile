FROM rust:alpine AS chef
WORKDIR /build
RUN apk add musl-dev curl bash podman
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall cargo-chef cross -y

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json

ARG TARGET=x86_64-unknown-linux-musl
# armv7-unknown-linux-musleabihf

RUN rustup target add ${TARGET}

ARG FEATURES=pid1,metrics
# Build dependencies - this is the caching Docker layer!
RUN --mount=type=cache,target=${CARGO_HOME}/registry \
    cargo chef cook --release --recipe-path recipe.json --features=$FEATURES --target=${TARGET}
# Build application
COPY . .
RUN --mount=type=cache,target=${CARGO_HOME}/registry \
    # --mount=type=cache,target=target \
    cargo build --release --features=$FEATURES --target=${TARGET}
RUN cp /build/target/${TARGET}/release/mcproxy /build/mcproxy

FROM scratch AS runner
LABEL org.opencontainers.image.source="https://github.com/dusterthefirst/mcproxy"
LABEL org.opencontainers.image.description="A reverse proxy for your Minecraft: Java Edition servers."
LABEL org.opencontainers.image.licenses="MPL-2.0"

COPY --from=builder /build/mcproxy /mcproxy
ENTRYPOINT ["/mcproxy"]
