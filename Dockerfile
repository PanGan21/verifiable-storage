# Multi-stage Dockerfile for verifiable-storage server

# Stage 1: Build stage
FROM rust:1.91-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace configuration files
COPY Cargo.toml ./
COPY crates ./crates
COPY bin/server/Cargo.toml ./bin/server/
COPY bin/client/Cargo.toml ./bin/client/
COPY tests/e2e/Cargo.toml ./tests/e2e/

# Create dummy source files for dependency compilation caching
# Note: Client dummy is needed because workspace includes it, but we only build the server binary
RUN mkdir -p bin/server/src bin/client/src && \
    echo "fn main() {}" > bin/server/src/main.rs && \
    echo "fn main() {}" > bin/client/src/main.rs

# Generate Cargo.lock (it will be created during the first cargo build if it doesn't exist)
# This ensures the build works whether Cargo.lock is committed or not
RUN cargo generate-lockfile || true

# Build dependencies only (this layer will be cached if dependencies don't change)
RUN cargo build --release --bin server 2>&1 | grep -E "(Compiling|Finished|error)" | tail -50 || true

# Copy actual server source code (this invalidates cache only when source changes)
COPY bin/server/src ./bin/server/src

# Force rebuild of server binary by touching the main.rs file
# This ensures Cargo rebuilds the server even if dependencies haven't changed
RUN touch bin/server/src/main.rs

# Build the actual server binary (client stays as dummy, not built)
RUN cargo build --release --bin server

# Stage 2: Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -m -u 1000 appuser

# Set working directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/server /app/server

# Change ownership to app user
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Run the server
# Command, port, and database URL are configured in docker-compose.yml
CMD ["./server"]
