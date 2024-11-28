#!/bin/bash

set -eo pipefail

IP_BASE="223.42.0.1/16"

/scripts/prepare-tap.sh -a "$IP_BASE" -o

berserker /etc/berserker/network-server.toml &

SERVER_PID=$!

berserker /etc/berserker/network-client.toml &

CLIENT_PID=$!

cleanup() {
    echo "Killing client ($CLIENT_PID) and server ($SERVER_PID)"

    kill -9 $CLIENT_PID
    kill -9 $SERVER_PID

    exit
}

trap cleanup SIGINT SIGKILL SIGABRT

wait $SERVER_PID
