<div align="center">
  <h1><code>componentize-go</code></h1>

  <p>
    <strong>Build WebAssembly components with Go</strong>
  </p>

  <strong>A <a href="https://bytecodealliance.org/">Bytecode Alliance</a> project</strong>

  <p>
    <a href="https://github.com/bytecodealliance/componentize-go/actions?query=workflow%3ACI"><img src="https://github.com/bytecodealliance/componentize-go/workflows/CI/badge.svg" alt="build status" /></a>
  </p>
</div>

This is a tool to convert a Go application to a [WebAssembly component](https://github.com/WebAssembly/component-model). It takes the following as input:

- a [WIT](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md) file or directory
- the name of a [WIT world](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md#wit-worlds) defined in the above file or directory
- the directory containing a Go module which targets said world

The output is a component which may be run using e.g. [`wasmtime`](https://github.com/bytecodealliance/wasmtime).

## Installation

### Using Go

Requires Go 1.27.1+

Add the following to your `go.mod` file and run `go mod tidy`:

```
tool github.com/bytecodealliance/componentize-go
```

In the same directory as your `go.mod` file, you can interact with the tool:

```sh
go tool componentize-go --help
```

Alternatively:

```sh
go install github.com/bytecodealliance/componentize-go@latest
```

It should be accessible via PATH:

```sh
componentize-go --help
```

### Download a release

You can download a specific release from the [release page](https://github.com/bytecodealliance/componentize-go/releases).

### Build from source

#### Prerequisites

- [**Rust toolchain**](https://rust-lang.org/) - Latest version

#### Run

```sh
cargo install --git https://github.com/bytecodealliance/componentize-go
```

## Usage

Please reference the `README.md` and `Makefile` files in each of the directories in [examples](./examples/).

## Build tags

When compiling your Go module, `componentize-go` passes `go build` the
`componentizego_async` build tag when the selected WIT world uses async
features (async functions, `stream`s, or `future`s).

SDKs can use `//go:build componentizego_async` to select between WASI 0.2 and
WASI 0.3 implementations of an API at compile time.

Any `-tags` you specify via the `GOFLAGS` environment variable are merged with
the tags above (rather than being overridden by them).
