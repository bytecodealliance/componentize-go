.PHONY: run build

build:
	componentize-go build

run: build
	wasmtime run -S p3,inherit-network -W component-model-async main.wasm
