package cli

import (
	"pkg/bindings/exports/export_wasi_cli_run"

	// NOTE: The application will not compile unless the
	// generated wit_exports are imported like this
	_ "pkg/bindings/exports/wit_exports"

	witTypes "go.bytecodealliance.org/pkg/wit/types"
)

type Component interface {
	Run() error
}

func RegisterExports(c Component) {
	export_wasi_cli_run.Exports.Run = func() witTypes.Result[witTypes.Unit, witTypes.Unit] {
		if err := c.Run(); err != nil {
			panic(err)
		}

		return witTypes.Ok[witTypes.Unit, witTypes.Unit](witTypes.Unit{})
	}
}
