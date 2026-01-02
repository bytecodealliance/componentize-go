# componentize-go

> [!Caution]
> This is a work-in-progress.


This is a tool to convert a Go application to a [WebAssembly component](https://github.com/WebAssembly/component-model). It takes the following as input:

- a [WIT](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md) file or directory
- the name of a [WIT world](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md#wit-worlds) defined in the above file or directory
- the directory containing a Go module which targets said world

The output is a component which may be run using e.g. [`wasmtime`](https://github.com/bytecodealliance/wasmtime).

## Installation
### Prerequisites
- [**Rust toolchain**](https://rust-lang.org/) - Latest version

### Run
```sh
cargo install --git https://github.com/asteurer/componentize-go
```
