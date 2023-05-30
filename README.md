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

Every workload is executed via set of worker processes, that are distributed
among available system CPU cores.

\[1\]: https://en.wikipedia.org/wiki/Poisson_point_process

\[2\]: "Open versus closed: A cautionary tale". Schroeder, B., Wierman, A. and
Harchol-Balter, M., USENIX. 2006.

\[3\]: https://en.wikipedia.org/wiki/Zipf%27s_law
