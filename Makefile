.DEFAULT_GOAL = all

ifeq ($(BERSERKER_TAG),)
BERSERKER_TAG=$(shell git describe --tags --abbrev=10 --dirty)
endif

.PHONY: all
all:
	docker build -t berserker -f Containerfile .
	docker build -t berserker-test -f Containerfile.test .
	docker run --privileged berserker-test

.PHONY: build-network
build-berserker-network:
	docker build -t berserker-network scripts/network

.PHONY: tag
tag:
	@echo "$(BERSERKER_TAG)"
