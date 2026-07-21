//! Embedded wasmtime setup helper functions for tests.

use anyhow::anyhow;
use wasmtime::{Config, Engine};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

pub struct State {
    ctx: WasiCtx,
    table: ResourceTable,
    stdout: MemoryOutputPipe,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl State {
    pub fn new() -> Self {
        let capacity = 1024 * 1024; // 1 MB
        let stdout = MemoryOutputPipe::new(capacity);
        let ctx = WasiCtx::builder().stdout(stdout.clone()).build();
        State {
            ctx,
            table: ResourceTable::new(),
            stdout,
        }
    }

    /// Bytes the guest has written to stdout.
    pub fn stdout(&self) -> Vec<u8> {
        self.stdout.contents().to_vec()
    }
}

/// Wasmtime engine configured with component model and async support
pub fn engine() -> anyhow::Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    config.wasm_component_model_implements(true);
    Engine::new(&config).map_err(|e| anyhow!(e))
}
