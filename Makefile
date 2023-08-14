.DEFAULT_GOAL = all

ifeq ($(BERSERKER_TAG),)
BERSERKER_TAG=$(shell git describe --tags --abbrev=10 --dirty)
endif


.PHONY: all
all:
	docker build -t builder -f Dockerfile.build .
	docker build -t berserker .


.PHONY: tag
tag:
	@echo "$(BERSERKER_TAG)"
