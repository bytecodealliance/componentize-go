# Building SDKs for componentize-go applications

While feasible, it can be tedious to work directly with WIT types in Go. In most cases, it's better to have an SDK layer that handles the conversions from WIT types to idiomatic Go types. This example demonstrates the ways that we handle WIT `imports` and `exports` in order to have shared code feel natural for Go developers to use.

The [application](component/main.go) that's being demonstrated imports the TCP portion of `wasi:sockets` and defines a Run export for `wasi:cli`, both of which have been wrapped in an SDK (see the `pkg` directory).

## componentize-go.toml

```toml
# componentize-go.toml file structure

# The FULL names of the default WIT worlds
worlds = ["example:sdk/test", "foo:bar/baz"]
# The paths in which the WIT files are stored.
wit_paths = ["wit", "../other_wit_path"]
```

Notice in the [component/Makefile](component/Makefile) how the build command is simply `componentize-go build`. Contrast this with the other examples which explicitly specify WIT worlds and paths to the WIT files in their build commands. The [pkg/componentize-go.toml](pkg/componentize-go.toml) file is what enables this behavior.

When building a component, componentize-go will search the go.mod file's dependencies' respective repositories for a componentize-go.toml file in the root. This file indicates where the WIT files are stored and the default worlds that are to be used.

You can override the default worlds via the command line. Note that doing so causes componentize-go to ignore all componentize-go.toml world definitions. You will need to explicitly list every WIT world the component requires.

## Defining Imports

Imports are straightforward since componentize-go generates the bindings. To see how imports are abstracted, see the [sockets package](pkg/sockets/sockets.go).

## Defining Exports

Exports require a bit more structure than imports because we need to have a translation layer for idiomatic, user-defined export functions and the raw WIT function signatures the compiler will recognize.

**Step 1: Define an Exports variable** ([`pkg/bindings/exports/export_wasi_cli_run/wit_bindings.go`](pkg/bindings/exports/export_wasi_cli_run/wit_bindings.go))

We hand-write an `Exports` variable that contains an uninitialized slot for each function export. At startup, something will need to assign a function to each slot; otherwise, the export panics at runtime. We do it this way because the [generated export bindings](pkg/bindings//exports/wit_exports/wit_exports.go) expect a `Run` function in the `bindings/export_wasi_cli_run` package, and we want to avoid making manual edits to any of the generated files.

**Step 2: The SDK wrapper** ([`pkg/cli/cli.go`](pkg/cli/cli.go))

The SDK defines a `RegisterExports` function that both assigns to the generated `Exports` slots and handles conversion between idiomatic Go types (e.g. `error`) and the raw WIT types (e.g. `witTypes.Option[string]`) the bindings expect.

Note the required blank import of `wit_exports`, as the component will not compile without it.

**Step 3: The application** ([`component/main.go`](component/main.go))

The application implements the required function export(s) and calls `RegisterExports` from `init()`. The `main()` function is left empty to make the compiler happy.
