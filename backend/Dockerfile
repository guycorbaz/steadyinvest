FROM rust:latest as builder

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release -p backend

FROM debian:trixie-slim

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /usr/src/app/target/release/backend-cli /app/backend-cli
COPY --from=builder /usr/src/app/backend/config /app/config

ENV LOCO_ENV=production
EXPOSE 5150

CMD ["/app/backend-cli", "start"]
