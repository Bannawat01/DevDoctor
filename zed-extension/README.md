# DevDoctor Zed extension

This is a minimal MCP context-server extension. It delegates all diagnostics
to the separately installed `devdoctor-mcp` binary and does not duplicate core
logic or modify project/environment state.

## Development

Install the current WASI target, then use Zed's **Install Dev Extension**:

```text
rustup target add wasm32-wasip2
cargo build --target wasm32-wasip2
```

The `devdoctor-mcp` executable must be available on the host PATH. The
extension does not download binaries or embed credentials. Registry publishing
and real Zed Agent verification remain separate release gates.
