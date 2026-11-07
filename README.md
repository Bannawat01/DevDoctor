# DevDoctor

Local, read-only diagnostics for Rust, Node, Docker, ports, and environment configuration.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

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

`docs/zed-mcp.json` is a portable template. Replace its `command` value with the absolute path to your own `devdoctor-mcp.exe`; it never points to the author's machine.

## Windows binary

Download `devdoctor-windows-x86_64.zip` from the GitHub prerelease, extract it, and run `devdoctor.exe doctor --path <project>`. Verify the accompanying SHA-256 checksum before use.

The ready-to-copy local configuration is also in `docs/zed-mcp.json`. In Zed, open **Settings → AI → MCP Servers → Add Local Server**, then confirm the server is active before asking the Agent to run `diagnose_project`.

## Privacy

DevDoctor makes no cloud calls or environment changes. Values from `.env` and URLs are redacted; output contains variable names and safe endpoint metadata only.
