generate-bindings:
	componentize-go --world wasip3-example bindings --format
	go mod tidy

build-component: generate-bindings
	componentize-go --world wasip3-example build

.PHONY: run
run: build-component
	wasmtime serve -Sp3,cli -Wcomponent-model-async main.wasm
