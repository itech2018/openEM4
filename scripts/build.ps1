$ErrorActionPreference = "Stop"
cargo build --workspace --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
