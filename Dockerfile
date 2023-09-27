FROM rust:slim-bookworm as builder
WORKDIR /usr/src/growatt_server
#COPY ./src/* ./src/*
#COPY Cargo.* .
COPY . .
RUN cargo build --release
#RUN cargo install --path .
#CMD ["./target/release/growatt_server"]

FROM debian:bookworm-slim
COPY --from=builder ./target/release/growatt_server ./server/growatt_server
EXPOSE 5279/tcp
CMD ["./server/growatt_server"]