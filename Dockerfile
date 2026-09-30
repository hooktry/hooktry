FROM rust:1.98.1-slim-bookworm AS build

WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/release/ortyo /usr/local/bin/ortyo

ENV ORTYO_BIND=0.0.0.0:8080
EXPOSE 8080

CMD ["ortyo", "hosted"]
