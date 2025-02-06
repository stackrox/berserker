#!/usr/bin/env bash
set -eou pipefail

stop() { echo "$*" 1>&2 ; exit 1; }

with_sudo() {
    if [ -z "${CI_RUN}"]; then
        command sudo "$@"
    else
        command "$@"
    fi
}

export CI_RUN=${CI:-""}

if [ -z "${CI_RUN}"]; then
    which sudo &>/dev/null || stop "Don't have sudo"
fi

which bpftrace &>/dev/null || stop "Don't have bpftrace"
which bpftool &>/dev/null || stop "Don't have bpftool"
which berserker &>/dev/null || stop "Don't have berserker"
which pkill &>/dev/null || stop "Don't have pkill"
which socat &>/dev/null || stop "Don't have socat"

if [ ! -d "tests/workers/network/" ]; then
  echo "Can't find test directory. Smoke tests have to be run from the project root directory"
fi

if [ -z "${CI_RUN}" ]; then
    # Needs elevated privileges to run bpftrace, bpftool and client berserker.
    # Ask for it and cache the credentials.
    sudo -v
fi

echo "Cleanup..."
rm -f /tmp/server.log
with_sudo rm -f /tmp/client.log
with_sudo rm -f /tmp/tcpaccept.log
# in case if it's still running from a previous run
with_sudo pkill berserker || true

# make berserkers verbose
export RUST_LOG=trace

# start the server before bpftrace, to skip first accept
echo "Starting the server..."
berserker tests/workers/network/workload.server.toml &> /tmp/server.log &

# wait until it's accepting connections
while ! echo test | socat stdio tcp4-connect:10.0.0.1:8081 ;
do
    echo "Wait for server";
    sleep 0.5;
done

echo "Starting bpftrace..."

with_sudo bpftrace tests/workers/network/sys_accept.bt &> /tmp/tcpaccept.log &

# let bpftrace attach probes
attempts=0

while ! with_sudo bpftool prog | grep -q bpftrace ;
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
with_sudo -E PATH=$PATH bash -c \
    'berserker tests/workers/network/workload.client.toml &> /tmp/client.log &'

# let it do some work
sleep 5;

echo "Stopping..."
with_sudo pkill berserker || true
with_sudo pkill bpftrace || true

echo "Verifying the results..."
ENDPOINTS=$(cat /tmp/tcpaccept.log | grep hit | wc -l || echo "")

if (( $ENDPOINTS > 0 )); then
    echo "PASS (${ENDPOINTS} seen connections)"

    rm -f /tmp/server.log
    with_sudo rm -f /tmp/client.log
    with_sudo rm -f /tmp/tcpaccept.log

    exit 0;
else
    echo "FAIL"
    cat /tmp/server.log
    cat /tmp/client.log
    cat /tmp/tcpaccept.log
    exit 1;
fi;
