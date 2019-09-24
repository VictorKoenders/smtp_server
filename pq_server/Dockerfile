FROM ubuntu:19.04

RUN apt update && \
    apt upgrade -y && \
    apt install libssl-dev openssl libpq-dev -y

COPY pq_server /pq_server
COPY identity.pfx /identity.pfx

ENTRYPOINT ["/pq_server"]
