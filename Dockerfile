FROM rust:1.61-buster as builder
RUN apt-get update && apt-get install -qyy cmake clang && apt-get clean

WORKDIR /usr/src/leaks_suite
COPY ./ ./
RUN cargo install --path ./leaks_bot

FROM debian:buster-slim

RUN apt-get update && apt-get install -qyy ca-certificates openssl && apt-get clean
COPY --from=builder /usr/local/cargo/bin/leaks_bot /usr/local/bin/leaks_bot
ADD https://publicsuffix.org/list/public_suffix_list.dat /opt/public_suffix_list.dat
