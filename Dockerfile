# Build stage
FROM rust:slim-trixie AS builder

# Create a new empty shell project
WORKDIR /usr/src/ward-rs

# Copy dependencies and build them first (caching)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy the actual source code and templates
COPY src ./src
COPY templates ./templates
COPY assets ./assets

# Ensure cargo rebuilds the binary
RUN touch src/main.rs
RUN cargo build --release

# Production stage
FROM debian:trixie-slim

WORKDIR /app

# Install necessary runtime dependencies (e.g., for sysinfo or SSL if needed)
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Run as a non-root user for better security
RUN useradd -m -s /bin/bash ward_user && chown -R ward_user /app
USER ward_user

# Copy the compiled binary and assets
COPY --from=builder /usr/src/ward-rs/target/release/ward ./ward
COPY --from=builder /usr/src/ward-rs/assets ./assets

# Create empty setup.ini or ensure it can be created
RUN touch setup.ini && chmod 666 setup.ini

# Expose port
EXPOSE 4000

# Run the binary
ENTRYPOINT ["./ward"]
