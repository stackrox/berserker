# Berserker

Workload generator for benchmarking ACS Collector, that supports following
workloads:

* Processes based workload to simulate systems with large number of processes.
  Processes spawning and exiting is modelled by a Poisson process \[1\] with
  specified rates of arrival and departure. In this way the tool can model an
  open system with more realistic latencies \[2\].

  The workload can function in two ways: either only forking the current
  process, or do fork plus exec with a randomly generated parameters into a
  small binary, which will immediately exit. The first approach will hit the
  path where process events are getting filtered out on the Collector level,
  the second will make Collector fully process events and send them further.

* Endpoint based workload to simulate systems with large number of network
  listening activity. Every worker opens and listens on a number of ports,
  modelled by Zipf \[3\] or uniform distributions (to cover extreme cases, when
  one process has significantly more endpoints than others).

* Syscall based workload to evaluate certain type of edge cases. Intended to
  verify an overhead where normally Collector doesn't stay in the way, but
  could be with the vanilla Falco. Similarly to the process based workload,
  syscalls are also modelled by a Poisson process.

* Network based workload to simulate systems with large number of open
  connections coming from variety of different addresses. To reduce amount of
  resources needed for such simulation and be able to pretend a connection is
  coming from a particular address, a tun device is used to craft an external
  client connection in the userspace.

* BPF based workload, which creates a specified number of simple BPF programs,
  attached to a specified tracepoint. This allows to simulate program
  contention on the same attachment point.

# Configuration

There are few ways to tune the configuration:

* If nothing is provided, Berserker will search for a file at
  `/etc/berserker/workload.toml`

* The target configuration can be provided via the first commandline argument,
  i.e. `berserker workload.toml`

* The configuration could be further adjusted via environment variables, e.g.
  `BERSERKER__WORKLOAD__ARRIVAL_RATE=1`. Such a variable have to start with the
  prefix `BERSERKER__` and use `__` to change nesting level.

You can specify which workload you want to use via option `type`. For every
type of workload there is an example in the `workloads/` directory.

A workload can be executed using one or more worker processes. By default one
worker is spawn per CPU core and and pinned to it to fully utilize system
resources. For some workload it might be needed to have a specified number of
worker instead without any implied affinity -- in this case they could be
configured usign option `per_core` and `workers`.

# How to contribute

* Make sure you've got recent enough version of Rust compiler. At the moment
  the minimal required version is 1.80 .

* Build project either directly using `cargo build`, or using containerized
  version implemented as a make `all` target.

* Find out what you need to change. The rule of thumb: if it has something to
  do with a specific workload type, it goes into a corresponding workload
  module under the `worker` directory; if it's a general improvement, feel free
  to modify anything outside workers.

* Do all sorts of hacking. Use `RUST_LOG=info` environment variable for getting
  verbose output when troubleshooting.

* Make sure tests are passing, `cargo test`.

* Run linter and formatter, `cargo clippy` & `cargo fmt`.

\[1\]: https://en.wikipedia.org/wiki/Poisson_point_process

\[2\]: "Open versus closed: A cautionary tale". Schroeder, B., Wierman, A. and
Harchol-Balter, M., USENIX. 2006.

\[3\]: https://en.wikipedia.org/wiki/Zipf%27s_law
