generate-bindings:
	componentize-go --world wasip2-example bindings

build-component: generate-bindings
	componentize-go --world wasip2-example componentize

.PHONY: run
run: build-component
	wasmtime serve -Sp3,cli main.wasm
