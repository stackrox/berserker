#!/usr/bin/env bash
set -eou pipefail

stop() { echo "$*" 1>&2 ; exit 1; }

which sudo &>/dev/null || stop "Don't have sudo"
which bpftrace &>/dev/null || stop "Don't have bpftrace"
which bpftool &>/dev/null || stop "Don't have bpftool"
which berserker &>/dev/null || stop "Don't have berserker"
which stub &>/dev/null || stop "Don't have random process tool"
which pkill &>/dev/null || stop "Don't have pkill"

if [ ! -d "tests/workers/script/unit" ]; then
  echo "Can't find test directory. Smoke tests have to be run from the project root directory"
fi

# Needs elevated privileges to run bpftrace and bpftool. Ask for it and cache
# the credentials.
sudo -v

echo "Cleanup..."
rm -f /tmp/berserker.log
sudo rm -f /tmp/events.log

# in case if it's still running from a previous run
pkill berserker || true

# make berserkers verbose
export RUST_LOG=trace

echo "Starting bpftrace..."
sudo bpftrace tests/workers/script/unit/syscalls.bt &> /tmp/events.log &

# let bpftrace attach probes
attempts=0

while ! sudo bpftool prog | grep -q bpftrace ;
do
    if [[ "$attempts" -gt 20 ]]; then
       echo "Can't find bpftool after ${attempts} attempts."
       exit 1
    fi;

    attempts=$((attempts+1))
    echo "Wait for bpftrace";
    sleep 0.5;
done

echo "Starting berserker..."
berserker -f tests/workers/script/unit/workload.ber &> /tmp/berserker.log

echo "Stopping..."
pkill berserker || true
sudo pkill bpftrace || true

echo "Verifying the results..."
if ! grep -q -E 'exec .*/stub' /tmp/events.log; then
    echo "FAIL: no task instruction"
    cat /tmp/berserker.log
    cat /tmp/events.log
    exit 1;
fi

if ! grep -q -E 'openat /tmp/tests/test' /tmp/events.log; then
    echo "FAIL: no open instruction"
    cat /tmp/berserker.log
    cat /tmp/events.log
    exit 1;
fi

if ! grep -E 'openat /tmp/tests/.*' /tmp/events.log | grep -q -v '/tmp/tests/test'; then
    echo "FAIL: no open random path instruction"
    cat /tmp/berserker.log
    cat /tmp/events.log
    exit 1;
fi

if ! grep -q -E 'connect .*' /tmp/events.log; then
    echo "FAIL: no ping instruction"
    cat /tmp/berserker.log
    cat /tmp/events.log
    exit 1;
fi

if ! grep -q -E 'sendto .*' /tmp/events.log; then
    echo "FAIL: ping instruction did not work"
    cat /tmp/berserker.log
    cat /tmp/events.log
    exit 1;
fi

echo "PASS"

rm -f /tmp/berserker.log
sudo rm -f /tmp/events.log

exit 0;
