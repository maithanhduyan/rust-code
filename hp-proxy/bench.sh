#!/bin/bash
set -e

echo "=== Killing old processes ==="
pkill -9 hp-echo 2>/dev/null || true
pkill -9 hp-proxy 2>/dev/null || true
sleep 1

echo "=== Starting echo server ==="
./target/release/hp-echo > /dev/null 2>&1 &
ECHO_PID=$!
sleep 1

echo "=== Starting proxy ==="
./target/release/hp-proxy > /dev/null 2>&1 &
PROXY_PID=$!
sleep 2

echo "=== Testing connection ==="
curl -s http://127.0.0.1:8080/test || { echo "FAILED"; exit 1; }
echo ""
echo "Connection OK!"

echo ""
echo "=== Running benchmark (wrk -t8 -c500 -d20s) ==="
wrk -t8 -c500 -d20s http://127.0.0.1:8080/test

echo ""
echo "=== Cleanup ==="
kill $PROXY_PID 2>/dev/null || true
kill $ECHO_PID 2>/dev/null || true

echo "Done!"
