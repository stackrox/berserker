all:
	docker build -t builder -f Dockerfile.build .
	docker build -t berserker .
