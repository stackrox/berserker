FROM builder:latest as builder

FROM registry.fedoraproject.org/fedora:41

RUN mkdir /etc/berserker

COPY --from=builder /berserker/target/release/berserker /usr/local/bin/berserker
COPY --from=builder /berserker/workload.toml /etc/berserker/workload.toml
COPY --from=builder /berserker/stub /usr/local/bin/stub

ENV PATH="${PATH}:/usr/local/bin"

ENTRYPOINT berserker
