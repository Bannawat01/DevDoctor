# DevDoctor

mcp-name: io.github.Bannawat01/devdoctor

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

Registry preparation and the publishing checklist are documented in [`docs/zed-mcp-registry.md`](docs/zed-mcp-registry.md). `devdoctor-mcp` 0.1.1 is published on crates.io; the 0.1.2 metadata is prepared for the next patch release and registry update.

The minimal Zed extension source is in [`zed-extension/`](zed-extension/). It delegates to a separately installed `devdoctor-mcp` binary on PATH; it is not yet published or verified in the Zed Agent UI.

`docs/zed-mcp.json` is a portable template. Replace its `command` value with the absolute path to your own `devdoctor-mcp.exe`; it never points to the author's machine.

Docker diagnostics are read-only. When a Compose project is detected and the Docker engine is reachable, DevDoctor inspects `docker compose ps --all` and reports services that are exited or unhealthy. It never starts, stops, restarts, or removes containers. An unavailable engine is reported separately from a service failure.

## Windows binary

Download `devdoctor-windows-x86_64.zip` from the GitHub prerelease, extract it, and run `devdoctor.exe doctor --path <project>`. Verify the accompanying SHA-256 checksum before use.

The ready-to-copy local configuration is also in `docs/zed-mcp.json`. In Zed, open **Settings → AI → MCP Servers → Add Local Server**, then confirm the server is active before asking the Agent to run `diagnose_project`.

## Privacy

DevDoctor makes no cloud calls or environment changes. Values from `.env` and URLs are redacted; output contains variable names and safe endpoint metadata only.

## Port ownership scope

When a configured local TCP port is occupied, DevDoctor may add the owning PID and process name to the port finding. On Windows this uses `GetExtendedTcpTable` with `AF_INET`; on Linux it reads `/proc/net/tcp` and process file descriptors. IPv6 ownership is not currently implemented, so an occupied IPv6 listener may have no owner evidence.

Executable metadata may be queried internally when the operating system permits it. Full executable paths are deliberately withheld from CLI and MCP findings; only `executable_path_available=true|false` is exposed. Permission restrictions and unsupported platforms preserve the port conflict finding without terminating or modifying processes.
