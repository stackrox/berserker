#!/usr/bin/env bash
set -eou pipefail

stop() { echo "$*" 1>&2 ; exit 1; }

which bpftrace &>/dev/null || stop "Don't have bpftrace"
which bpftool &>/dev/null || stop "Don't have bpftool"
which berserker &>/dev/null || stop "Don't have berserker"
which pkill &>/dev/null || stop "Don't have pkill"
which socat &>/dev/null || stop "Don't have socat"

echo "Cleanup..."
rm -f /tmp/server.log
rm -f /tmp/client.log
rm -f /tmp/tcpaccept.log
# in case if it's still running from a previous run
pkill berserker || true

# make berserkers verbose
#export RUST_LOG=trace

TEST_DIR="$(cd -- "$( dirname -- "${BASH_SOURCE[0]}")"  &> /dev/null && pwd)"

# start the server before bpftrace, to skip first accept
echo "Starting the server..."
berserker "${TEST_DIR}/workload.server.toml" &> /tmp/server.log &

# wait until it's accepting connections
while ! echo test | socat stdio tcp4-connect:10.0.0.1:8081 ;
do
    echo "Wait for server";
    sleep 0.5;
done

echo "Starting bpftrace..."
bpftrace "${TEST_DIR}/sys_accept.bt" &> /tmp/tcpaccept.log &

# let bpftrace attach probes
attempts=0

while ! bpftool prog | grep -q bpftrace ;
do
    if [[ "$attempts" -gt 20 ]]; then
       echo "Can't find bpftool after ${attempts} attempts."
       exit 1
    fi;

    attempts=$((attempts+1))
    echo "Wait for bpftrace";
    sleep 0.5;
done

echo "Starting the client..."
berserker "${TEST_DIR}/workload.client.toml" &> /tmp/client.log &

# let it do some work
sleep 5;

echo "Stopping..."
pkill berserker || true
pkill bpftrace || true

echo "Verifying the results..."
ENDPOINTS=$(cat /tmp/tcpaccept.log | grep hit | wc -l || echo "")

if (( $ENDPOINTS > 0 )); then
    echo "PASS (${ENDPOINTS} seen connections)"

    rm -f /tmp/server.log
    rm -f /tmp/client.log
    rm -f /tmp/tcpaccept.log

    exit 0;
else
    echo "FAIL"
    cat /tmp/server.log
    cat /tmp/client.log
    cat /tmp/tcpaccept.log
    exit 1;
fi;
