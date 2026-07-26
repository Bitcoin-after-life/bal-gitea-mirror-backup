# =============================================================================
# Multi-stage Dockerfile for bal-server + bal-pusher
# Security: non-root user, minimal runtime, tini as PID 1
# =============================================================================

# ---------------------------------------------------------------------------
# Stage 1: Builder
# ---------------------------------------------------------------------------
FROM rust:1.95-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libzmq5-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies: copy Cargo.toml first, create dummy src to build deps
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src/bin && \
    echo 'fn main() {}' > src/bin/bal-server.rs && \
    echo 'fn main() {}' > src/bin/bal-pusher.rs && \
    echo '' > src/lib.rs && \
    echo '' > src/db.rs && \
    echo '' > src/xpub.rs && \
    echo '' > src/validation.rs && \
    cargo build --release --bin bal-server --no-default-features --features server 2>/dev/null || true && \
    cargo build --release --bin bal-pusher --no-default-features --features pusher 2>/dev/null || true && \
    rm -rf src target/release/.fingerprint target/release/deps/*bal_server*

# Copy real source and build each binary with only its required features
COPY src/ src/
RUN cargo build --release --bin bal-server --no-default-features --features server && \
    cargo build --release --bin bal-pusher --no-default-features --features pusher && \
    strip target/release/bal-server target/release/bal-pusher

# ---------------------------------------------------------------------------
# Stage 2: Runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies + tini for PID 1
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    libzmq5 \
    libsqlite3-0 \
    ca-certificates \
    curl \
    tini \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Copy binaries from builder
COPY --from=builder /build/target/release/bal-server /usr/local/bin/bal-server
COPY --from=builder /build/target/release/bal-pusher /usr/local/bin/bal-pusher

# Create dedicated non-root user
RUN groupadd -g 1000 bal && \
    useradd -u 1000 -g bal -s /usr/sbin/nologin -M bal && \
    mkdir -p /var/bal /var/bal/.bitcoin && \
    chown -R bal:bal /var/bal && \
    chmod 700 /var/bal

# Copy entrypoint
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Use tini as PID 1 for proper signal handling
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/entrypoint.sh"]

# Data directory (mount as volume)
VOLUME ["/var/bal"]

# bal-server port (bind to 127.0.0.1 via env, expose for reverse proxy)
EXPOSE 9137

# Default environment (override at runtime)
ENV RUST_LOG=info \
    BAL_SERVER_BIND_ADDRESS=127.0.0.1 \
    BAL_SERVER_BIND_PORT=9137 \
    BAL_SERVER_DB_FILE=/var/bal/bal.db \
    BAL_PUSHER_DB_FILE=/var/bal/bal.db \
    BAL_SERVER_URL=http://127.0.0.1:9137 \
    BAL_SERVER_PUB_KEY_PATH=/var/bal/public_key.pem \
    SSL_KEY_PATH=/var/bal/private_key.pem

# Health check: verify bal-server is responding
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://127.0.0.1:9137/ || exit 1
