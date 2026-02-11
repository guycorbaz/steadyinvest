FROM rust:latest as builder

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release -p backend

FROM debian:trixie-slim

RUN apt-get update && apt-get install -y libssl-dev ca-certificates fonts-dejavu-core fonts-dejavu-extra && rm -rf /var/lib/apt/lists/* \
    && cd /usr/share/fonts/truetype/dejavu \
    && ln -s DejaVuSans.ttf DejaVuSans-Regular.ttf \
    && ln -s DejaVuSans-BoldOblique.ttf DejaVuSans-BoldItalic.ttf \
    && ln -s DejaVuSans-Oblique.ttf DejaVuSans-Italic.ttf

WORKDIR /app
COPY --from=builder /usr/src/app/target/release/backend-cli /app/backend-cli
COPY --from=builder /usr/src/app/backend/config /app/config

ENV LOCO_ENV=production
EXPOSE 5150

CMD ["/app/backend-cli", "start"]
