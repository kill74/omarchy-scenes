FROM rust:1.99 AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/scenes /usr/local/bin/scenes
COPY --from=builder /src/target/release/scenesd /usr/local/bin/scenesd
ENTRYPOINT ["scenesd"]
