.PHONY: build gui install test fmt

build:
	cargo build -p smwapt

gui:
	cmake -S gui -B build/gui -GNinja
	cmake --build build/gui

install:
	scripts/install-local

test:
	cargo test

fmt:
	cargo fmt --all
