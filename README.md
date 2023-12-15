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

* Syscall based workload to evaluate certain type of edge cases. For now only
  one syscall, `getpid` is included, to verify an overhead where normally
  Collector doesn't stay in the way, but could be with the vanilla Falco.
  Similarly to the process based workload, syscalls are also modelled by a
  Poisson process.

Every workload is executed via set of worker processes, that are distributed
among available system CPU cores to fully utilize system resources.

# How to contribute

* Make sure you've got recent enough version of Rust compiler. At the moment
  the minimal required version is 1.71 .

* Build project either directly using `cargo build`, or using containerized
  version implemented as a make `all` target.

* Find out what you need to change. The rule of thumb: if it has something to
  do with a specific workload type, it goes into a corresponding workload
  module under the `worker` directory; if it's a general improvement, feel free
  to modify anything outside workers.

* Do all sorts of hacking. Use `RUST_LOG=info` environment variable for getting
  verbose output when troubleshooting.

* Make sure tests are passing, `cargo test`.

* Run linter, `cargo clippy`.

# Adding a new worker

If your goal is to implement a new worker, take a look at `TemplateWorker`.
It's a dummy worker implementation, the sole purpose of which is to show all
parts that have to be touched to wire up a custom worker. Copy the template
over under a new name, and you'll get a decent ground for development.

\[1\]: https://en.wikipedia.org/wiki/Poisson_point_process

\[2\]: "Open versus closed: A cautionary tale". Schroeder, B., Wierman, A. and
Harchol-Balter, M., USENIX. 2006.

\[3\]: https://en.wikipedia.org/wiki/Zipf%27s_law
