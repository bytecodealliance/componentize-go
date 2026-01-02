use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    componentize_go::command::run(env::args_os())
}
