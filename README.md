# DevDoctor

Local, read-only diagnostics for Rust, Node, Docker, ports, and environment configuration.

## Run

```powershell
cargo run -p devdoctor-cli -- doctor --path .
cargo run -p devdoctor-cli -- doctor --json
cargo run -p devdoctor-cli -- check rust
cargo run -p devdoctor-cli -- explain ENV-DATABASE-001
```

Exit codes: `0` healthy, `1` diagnostic errors/warnings, `2` invalid invocation or inspection failure.

## MCP / Zed

Build the release server, then add it in Zed: Settings → AI → MCP Servers → Add Local Server.

```json
{
  "context_servers": {
    "devdoctor": {
      "command": "C:/absolute/path/to/devdoctor-mcp.exe",
      "args": []
    }
  }
}
```

The stdio server exposes `diagnose_project`, `check_toolchain`, `check_environment`, `check_ports`, and `explain_finding`. It accepts only local existing directories and never runs repository scripts or changes environment state.

## Privacy

DevDoctor makes no cloud calls or environment changes. Values from `.env` and URLs are redacted; output contains variable names and safe endpoint metadata only.
