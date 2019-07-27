FROM ubuntu:19.04

RUN apt update && \
    apt upgrade -y && \
    apt install libssl-dev openssl libpq-dev -y

COPY pq_server /pq_server

ENTRYPOINT ["/pq_server"]
