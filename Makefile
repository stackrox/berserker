.DEFAULT_GOAL = all

ifeq ($(BERSERKER_TAG),)
BERSERKER_TAG=$(shell git describe --tags --abbrev=10 --dirty)
endif

.PHONY: all
all:
	docker build -t berserker-stable -f Containerfile .
	docker build -t berserker-test -f Containerfile.test .
	docker run --privileged berserker-test

	docker build -t berserker-nightly -f Containerfile --build-arg=RUST_VERSION=nightly .
	docker build -t berserker-test -f Containerfile.test --build-arg=RUST_VERSION=nightly .
	docker run --privileged berserker-test

	docker tag berserker-stable berserker

.PHONY: build-network
build-berserker-network:
	docker build -t berserker-network scripts/network

.PHONY: tag
tag:
	@echo "$(BERSERKER_TAG)"
