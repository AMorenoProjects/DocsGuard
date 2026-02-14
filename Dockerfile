# Stage 1: Build
FROM rust:1.83-slim AS builder

WORKDIR /build

# Instalar dependencias del sistema para tree-sitter (compilación C)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Cachear dependencias: copiar solo manifiestos primero
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Compilar el proyecto real
COPY . .
RUN cargo build --release --locked && strip target/release/docsguard

# Stage 2: Runtime mínimo
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/docsguard /usr/local/bin/docsguard

# Usuario no-root
RUN useradd -m docsguard
USER docsguard
WORKDIR /workspace

ENTRYPOINT ["docsguard"]
CMD ["--help"]
