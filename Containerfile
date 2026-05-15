FROM registry.fedoraproject.org/fedora:43 as builder

RUN dnf install -y \
    rust \
    cargo \
    clippy \
    rustfmt \
    # for stub \
    nasm \
    # for script jit \
    llvm-devel \
    zlib-devel \
    libxml2-devel \
    libstdc++-static \
    # for bpf \
    clang \
    kernel-devel \
    libbpf-devel

ADD ./ /berserker/

WORKDIR /berserker/

RUN nasm -f elf64 -o stub.o stub.asm && ld -o stub stub.o

RUN cargo fmt --check

RUN cargo clippy -- -D warnings

ARG CARGO_FEATURES=""
RUN cargo build -r ${CARGO_FEATURES:+--features $CARGO_FEATURES}

# Test will require stub binary to be available
ENV PATH="${PATH}:/berserker:/berserker/target/release"

RUN cargo test ${CARGO_FEATURES:+--features $CARGO_FEATURES}

FROM registry.fedoraproject.org/fedora:43

RUN mkdir /etc/berserker

COPY --from=builder /berserker/target/release/berserker /usr/local/bin/berserker
COPY --from=builder /berserker/workload.toml /etc/berserker/workload.toml
COPY --from=builder /berserker/stub /usr/local/bin/stub

COPY --from=builder /berserker/profile-entrypoint.sh /usr/local/bin/profile-entrypoint.sh

ENV PATH="${PATH}:/usr/local/bin"

ENTRYPOINT berserker
