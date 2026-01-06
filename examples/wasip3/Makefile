GO := go-$(shell uname -s | tr 'A-Z' 'a-z')-$(shell uname -m | sed -e 's/aarch64/arm64/' -e 's/x86_64/amd64/')-bootstrap

$(GO)/bin/go:
	curl -OL https://github.com/dicej/go/releases/download/go1.25.5-wasi-on-idle/$(GO).tbz
	tar xf $(GO).tbz

generate-bindings:
	componentize-go --world wasip3-example bindings --format

build-component: generate-bindings $(GO)/bin/go
	componentize-go --world wasip3-example componentize --go $(GO)/bin/go

.PHONY: run
run: build-component
	wasmtime serve -Sp3,cli -Wcomponent-model-async main.wasm
