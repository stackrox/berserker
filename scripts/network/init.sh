#!/bin/bash

set -eo pipefail

IP_BASE="${IP_BASE:-223.42.0.1/16}"

if [[ "$BERSERKER_CLIENT" == "1" ]]; then
    berserker /etc/berserker/network-client.toml &
    PID=$!
else
    /scripts/prepare-tap.sh -a "$IP_BASE" -o

    berserker /etc/berserker/network-server.toml &
    PID=$!
fi

cleanup() {
    echo "Killing ($PID)"

    kill -9 "$PID"

    exit
}

trap cleanup SIGINT SIGABRT

wait -n "$PID"
