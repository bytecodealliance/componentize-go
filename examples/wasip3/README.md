# `wasip3` Example
This demonstrates how to implement a `wasi:http@0.3.0-rc-2025-09-16` handler
using Go, based on a new `wit-bindgen-go` bindings generator which supports
idiomatic, goroutine-based concurrency on top of the [Component Model
concurrency
ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md).

As of this writing, not everything has been upstreamed and released, so this
relies on specific Git revisions of certain tools, plus [a patched version of
Go](https://github.com/dicej/go/releases/tag/go1.25.5-wasi-on-idle).  Once
everything is merged, we'll be able to switch to the upstream releases.

## Building and Running
### Prerequisites
- [**componentize-go**](https://github.com/bytecodealliance/componentize-go) - Latest version
- [**go**](https://github.com/dicej/go/releases/tag/go1.25.5-wasi-on-idle) - The [Makefile](./Makefile) installs the patched version of Go.
- [**wasmtime**](https://github.com/bytecodealliance/wasmtime)  - v40.0.2

This will build the dependencies, generate Go bindings from the
`wasi:http@0.3.0-rc-2025-09-16` WIT files, build the component, and run it using
`wasmtime serve`:

```shell
make run
```

While that's running, you can send a request from another shell:

```
curl -i http://127.0.0.1:8080/hello
```

If all goes well, you should see `hello, world!`.

You can also try the other endpoints, e.g. `/echo`, which does full-duplex
streaming:

```
curl -i -H 'content-type: text/plain' --data-binary @- http://127.0.0.1:8080/echo <<EOF
’Twas brillig, and the slithy toves
      Did gyre and gimble in the wabe:
All mimsy were the borogoves,
      And the mome raths outgrabe.
EOF
```

...and `/hash-all`, which concurrently downloads one or more URLs and streams the
SHA-256 hashes of their contents:

```
curl -i \
    -H 'url: https://webassembly.github.io/spec/core/' \
    -H 'url: https://www.w3.org/groups/wg/wasm/' \
    -H 'url: https://bytecodealliance.org/' \
    http://127.0.0.1:8080/hash-all
```

## Note
This was originally built by [@dicej](https://github.com/dicej) and has been adapted from the [original example](https://github.com/dicej/go-wasi-http-example/tree/main) to use componentize-go.
