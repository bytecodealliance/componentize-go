# `wasip2` Example

## Usage

### Prerequisites

- [**componentize-go**](https://github.com/bytecodealliance/componentize-go) - Latest version
- [**go**](https://go.dev/dl/) - v1.25+
- [**wasmtime**](https://github.com/bytecodealliance/wasmtime)  - v40.0.2

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
