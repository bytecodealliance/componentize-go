GO := go-$(shell uname -s | tr 'A-Z' 'a-z')-$(shell uname -m | sed -e 's/aarch64/arm64/' -e 's/x86_64/amd64/')-bootstrap

install-patched-go:
	@if [ ! -d "../../$(GO)" ]; then \
		curl -OL https://github.com/dicej/go/releases/download/go1.25.5-wasi-on-idle/$(GO).tbz --output-dir ../../; \
		tar xf ../../$(GO).tbz -C ../../; \
		rm ../../$(GO).tbz; \
	else \
		echo "Go bootstrap already exists at ../../$(GO), skipping download"; \
	fi

generate-bindings: install-patched-go
	componentize-go --world wasip3-example bindings --format

build-component: generate-bindings
	componentize-go --world wasip3-example componentize --go ../../$(GO)/bin/go

.PHONY: run
run: build-component
	wasmtime serve -Sp3,cli -Wcomponent-model-async main.wasm
