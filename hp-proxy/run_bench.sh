#!/bin/bash
set -e

echo "Building HP Proxy..."
cargo build --release

echo ""
echo "Starting echo server..."
./target/release/hp-echo &
ECHO_PID=$!
sleep 1

echo "Starting proxy..."
./target/release/hp-proxy &
PROXY_PID=$!
sleep 1

echo ""
echo "Running benchmark (100 connections, 10 seconds)..."
./target/release/hp-bench 127.0.0.1:8080 100 10

echo ""
echo "Cleaning up..."
kill $PROXY_PID 2>/dev/null || true
kill $ECHO_PID 2>/dev/null || true

echo "Done!"
