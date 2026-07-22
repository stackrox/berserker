FROM registry.fedoraproject.org/fedora:43 AS builder

ARG RUST_VERSION=stable

RUN dnf install -y \
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
    libbpf-devel && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --default-toolchain $RUST_VERSION --profile minimal

# $HOME is not evaluated here, any better way of getting the toolchain
# directory?
ENV PATH="${PATH}:/root/.cargo/bin"

RUN rustup component add rustfmt clippy

RUN if [ "${RUST_VERSION}" == "nightly" ]; then \
    rustup component add rust-src --toolchain nightly; \
    fi

ADD ./ /berserker/

WORKDIR /berserker/

RUN nasm -f elf64 -o stub.o stub.asm && ld -o stub stub.o

RUN cargo fmt --check

RUN cargo clippy -- -D warnings

RUN cargo build -r

# Test will require stub binary to be available
ENV PATH="${PATH}:/berserker:/berserker/target/release"

RUN if [ "${RUST_VERSION}" == "nightly" ]; then \
        TARGET=$(rustc --version --verbose | grep host | cut -d" " -f2) && \
        RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -Z build-std --target "$TARGET"; \
    else \
        cargo test; \
    fi

FROM registry.fedoraproject.org/fedora:43

RUN mkdir /etc/berserker

COPY --from=builder /berserker/target/release/berserker /usr/local/bin/berserker
COPY --from=builder /berserker/workload.toml /etc/berserker/workload.toml
COPY --from=builder /berserker/stub /usr/local/bin/stub

ENV PATH="${PATH}:/usr/local/bin"

ENTRYPOINT berserker
