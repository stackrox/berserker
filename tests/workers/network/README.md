A simple smoke test for network worker. The test spins two berserker instances,
one as a server and one as a client, then uses bpftrace to confirm that accept
syscall is getting triggered. There are few requirements to run the test:

* Since the networking configuration might differ between environments, it's
  necessary to invoke `scripts/network/prepare-tap.sh` with necessary arguments
  before the test.
* Test needs to be run with superuser privileges.
* Berserker binary to test needs to be available in PATH.
