#!/usr/bin/env bash
mount -t debugfs none /sys/kernel/debug

./tests/workers/script/unit/smoke.sh
