#!/usr/bin/env bash
set -eou pipefail

stop() { echo "$*" 1>&2 ; exit 1; }

which sudo &>/dev/null || stop "Don't have sudo"
which bpftrace &>/dev/null || stop "Don't have bpftrace"
which bpftool &>/dev/null || stop "Don't have bpftool"
which berserker &>/dev/null || stop "Don't have berserker"
which stub &>/dev/null || stop "Don't have random process tool"
which pkill &>/dev/null || stop "Don't have pkill"

if [ ! -d "tests/workers/processes/random" ]; then
  echo "Can't find test directory. Smoke tests have to be run from the project root directory"
fi

# Needs elevated privileges to run bpftrace and bpftool. Ask for it and cache
# the credentials.
sudo -v

echo "Cleanup..."
rm -f /tmp/berserker.log
sudo rm -f /tmp/processes.log
# in case if it's still running from a previous run
pkill berserker || true

# make berserkers verbose
export RUST_LOG=trace

echo "Starting berserker..."
berserker tests/workers/processes/random/workload.toml &> /tmp/berserker.log &

echo "Starting bpftrace..."
sudo bpftrace tests/workers/processes/random/sys_exec.bt &> /tmp/processes.log &

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

# let berserker do some work
sleep 5;

echo "Stopping..."
pkill berserker || true
sudo pkill bpftrace || true

echo "Verifying the results..."
PROCS=$(cat /tmp/processes.log | grep hit | wc -l || echo "")

if (( $PROCS > 0 )); then
    echo "PASS (${PROCS} seen processes)"

    rm -f /tmp/berserker.log
    sudo rm -f /tmp/processes.log

    exit 0;
else
    echo "FAIL"
    cat /tmp/berserker.log
    cat /tmp/processes.log
    exit 1;
fi;
