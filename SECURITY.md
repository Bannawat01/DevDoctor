# Security policy

## Reporting a vulnerability

Please do not open a public issue for an exploitable security problem. Use GitHub's private vulnerability reporting for this repository when available, or contact the maintainers through the repository owner profile.

Include a concise description, affected version, reproduction steps using synthetic data, and impact. Do not send real credentials, tokens, connection strings, private paths, or diagnostic output containing secrets.

## Security principles

DevDoctor is local-first and read-only by default. It does not upload diagnostics or modify the environment. Reports redact credentials and avoid executing repository scripts. Please report any secret disclosure, unsafe command execution, privilege escalation, or unbounded probe promptly.
