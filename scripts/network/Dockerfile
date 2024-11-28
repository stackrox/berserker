FROM berserker:latest

RUN dnf install -y which iproute bpftool procps iptables

COPY prepare-tap.sh /scripts/
COPY init.sh /scripts/
COPY workloads /etc/berserker/

ENTRYPOINT ["/scripts/init.sh"]
