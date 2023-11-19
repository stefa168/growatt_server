FROM rust:slim-bookworm as builder
WORKDIR /usr/src/growatt_server/
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo install --path .

FROM debian:bookworm-slim
WORKDIR /usr/local/bin/
COPY --from=builder /usr/local/cargo/bin/growatt_server /usr/local/bin/growatt_server
EXPOSE 5279/tcp
STOPSIGNAL SIGTERM
ENTRYPOINT ["./growatt_server"]