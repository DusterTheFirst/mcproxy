
FROM rust:alpine AS chef
WORKDIR /build
RUN apk add musl-dev
RUN cargo install cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG FEATURES=pid1,discovery-docker,telemetry,tokio-console

# ARG features

COPY --from=planner /build/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json --features $FEATURES
# Build application
COPY . .
RUN --mount=type=cache,target=${CARGO_HOME}/registry \
    # --mount=type=cache,target=target \
    cargo build --release --features $FEATURES

FROM scratch AS runner
COPY --from=builder /build/target/release/mcproxy /mcproxy
ENTRYPOINT ["/mcproxy"]
