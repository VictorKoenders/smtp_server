#!/bin/bash
set +e

cargo build --release
cp ../target/release/pq_server .
docker build -t pq_server .
docker save pq_server --output pq_server.tar
