# `wasip2` Example

## Usage

### Prerequisites

- [**componentize-go**](https://github.com/bytecodealliance/componentize-go) - Latest version
- [**go**](https://go.dev/dl/) - v1.25.9
- [**wasmtime**](https://github.com/bytecodealliance/wasmtime)  - v44.0.1

### Run

```sh
# Start the application
make run

# Invoke the application
curl localhost:8080
```

### Run unit tests

```sh
# Method 1: compile the tests into wasm modules and run them with wasmtime
make run-tests

# Method 2: run the tests directly with `go test`
make generate-bindings
go test ./unit_tests_should_pass ./unit_tests_should_fail
```

### Run `go vet`

The generated code will fail the default `go vet` analysis due to:
- How `unsafe.Pointer` is used
- How the WIT tuple type is represented in Go

If you must perform static analysis on the generated code, this is the workaround:

```sh
go vet -unsafeptr=false -composites=false ./...
```
