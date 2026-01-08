# Builder stage
FROM rust:1.75-slim as builder

WORKDIR /usr/src/app
COPY . .

RUN cargo install --path .

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates openssl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/usthingy /usr/local/bin/usthingy
COPY .env.example .env.example

# Start the bot
CMD ["usthingy"]
