#!/bin/bash

set -eo pipefail

IP_BASE="${IP_BASE:-223.42.0.1/16}"

/scripts/prepare-tap.sh -a "$IP_BASE" -o

berserker -c /etc/berserker/network-server.toml &

SERVER_PID=$!

sleep 1

berserker -c /etc/berserker/network-client.toml &

CLIENT_PID=$!

cleanup() {
    echo "Killing client ($CLIENT_PID) and server ($SERVER_PID)"

    kill -9 "$CLIENT_PID" 2>/dev/null
    kill -9 "$SERVER_PID" 2>/dev/null

    exit
}

trap cleanup SIGINT SIGABRT SIGTERM

wait "$SERVER_PID" "$CLIENT_PID"
