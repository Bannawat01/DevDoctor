# Zed MCP Registry metadata

Status: `PUBLISHED`; `devdoctor-core` and `devdoctor-mcp` `0.1.2` are live on crates.io and `io.github.Bannawat01/devdoctor@0.1.2` is active in the official MCP Registry. The publisher input is the root `server.json`; `docs/mcp-server.json` remains the documented copy.

DevDoctor is a local stdio MCP server. A registry entry should launch the
versioned `devdoctor-mcp` release binary for the user's platform and pass no
project content or credentials to a remote service.

## Metadata

| Field | Value |
|---|---|
| Server name | `devdoctor` |
| Repository | `https://github.com/Bannawat01/DevDoctor` |
| Transport | Local stdio |
| Tools | `diagnose_project`, `check_toolchain`, `check_environment`, `check_ports`, `explain_finding` |
| Permissions | Read-only local diagnostics |
| Telemetry | None |
| Supported platform | Windows x86_64 (stable release artifact) |
| Experimental platform | Linux x86_64 (CI-verified, packaging may vary) |

The registry distribution is the `devdoctor-mcp` crates.io package. GitHub ZIPs
remain the native/manual Windows release path.

The server must be distributed separately from a Zed extension when the
extension API requires downloading a binary. Do not embed credentials or a
machine-specific path in registry metadata.

## Manual Zed configuration

Install the published server, then configure it under **Settings > AI > MCP
Servers > Add Local Server**:

```powershell
cargo install devdoctor-mcp --version 0.1.2 --locked
```

```json
{
  "context_servers": {
    "devdoctor": {
      "command": "devdoctor-mcp",
      "args": []
    }
  }
}
```

On Windows, if Zed does not inherit Cargo's binary directory, use the absolute
path to `%USERPROFILE%/.cargo/bin/devdoctor-mcp.exe`. After adding the server,
confirm that all five tools appear in the Agent panel and call
`diagnose_project` against a local fixture. The server must not start
containers, install dependencies, execute repository scripts, or modify
environment state.

## Publishing checklist

- [x] Validate the official MCP Registry schema.
- [x] Install the crates.io package and confirm protocol tool discovery.
- [x] Invoke `diagnose_project` from the Zed Agent UI.
- [ ] Re-run all five tools from the Zed Agent UI after a fresh Cargo install.
- [x] Record `ZED_AGENT_VERIFIED` separately from protocol tests.
- [x] Prepare `docs/mcp-server.json` using the current official schema.
- [x] Publish `devdoctor-core` and `devdoctor-mcp` to crates.io.
- [x] Validate and publish metadata with `mcp-publisher`.
