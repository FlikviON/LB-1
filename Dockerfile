# Сборка и прогон вашего решения.
#   docker build -t pp-lab-rust .
#   docker run --rm pp-lab-rust info
FROM rust:1.90-slim-bookworm AS build

WORKDIR /src
COPY . .
RUN cargo test --release --all-targets \
 && cargo build --release --bin harness

FROM debian:bookworm-slim
COPY --from=build /src/target/release/harness /usr/local/bin/harness
ENTRYPOINT ["harness"]
CMD ["info"]
