mod compiler_ext;
mod operation_cost;
mod operation_depth;
mod plugins;

use anyhow::Result;

fn main() -> Result<()> {
    apollo_router::main()
}
