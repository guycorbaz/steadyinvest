FROM rust:latest as builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install --locked trunk
RUN rustup target add wasm32-unknown-unknown

WORKDIR /usr/src/app
COPY . .

# Pre-build dependencies to identify if cargo panics here
RUN cargo build --release -p frontend --target wasm32-unknown-unknown

WORKDIR /usr/src/app/frontend
RUN trunk build --release

FROM nginx:stable-alpine

COPY --from=builder /usr/src/app/frontend/dist /usr/share/nginx/html
COPY --from=builder /usr/src/app/frontend/public /usr/share/nginx/html/public
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
