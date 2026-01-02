generate-bindings:
    wit-bindgen go --world bindings --out-dir . wit

build-component: generate-bindings
    componentize-go --world bindings componentize

.PHONY: run
run: build-component
    wasmtime serve -Sp3,cli main.wasm
