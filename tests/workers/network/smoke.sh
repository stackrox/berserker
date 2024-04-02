#!/usr/bin/env bash

stop() { echo "$*" 1>&2 ; exit 1; }

which bpftrace &>/dev/null || stop "Don't have bpftrace"
which bpftool &>/dev/null || stop "Don't have bpftool"

if [ ! -d "tests/workers/network/" ]; then
  echo "Can't find test directory. Smoke tests have to be run from the project root directory"
fi

echo "Cleanup..."
rm -f /tmp/server.log
rm -f /tmp/client.log
rm -f /tmp/tcpaccept.log

echo "Starting bpftrace..."
bpftrace tests/workers/network/tcpaccept.bt &> /tmp/tcpaccept.log &
export BPFTRACE_PID=$!

# let bpftrace attach probes
while ! bpftool prog | grep -q bpftrace ;
do
    echo "Wait for bpftrace";
    sleep 0.5;
done

echo "Starting the server..."
berserker tests/workers/network/workload.server.toml &> /tmp/server.log &
export SERVER_PID=$!

echo "Starting the client..."
berserker tests/workers/network/workload.client.toml &> /tmp/client.log &
export CLIENT_PID=$!

# let it do some work
sleep 5;

echo "Stopping..."
pkill -P "${CLIENT_PID}"
pkill -P "${SERVER_PID}"
pkill -P "${BPFTRACE_PID}"

echo "Verifying the results..."
ENDPOINTS=$(cat /tmp/tcpaccept.log |\
                grep berserker |\
                awk '{print $4 " " $5}' |\
                sort | uniq -c | wc -l)

if (( $ENDPOINTS > 0 )); then
    echo "PASS"

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
