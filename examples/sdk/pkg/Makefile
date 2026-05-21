generate-bindings:
	@rm -rf bindings/imports
	@rm -rf bindings/exports/wit_exports
# The export_wasi_cli_run files are hand-written, and thus are not removed

	@componentize-go \
		--ignore-toml-files \
		bindings \
		--output bindings/imports \
		--pkg-name pkg/bindings/imports \
		--export-pkg-name pkg/bindings/exports \
		--format

	@mv bindings/imports/wit_exports bindings/exports