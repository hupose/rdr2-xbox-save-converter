# Security Policy

## Data handling

The converter is offline. It does not authenticate to Xbox, make network requests, upload saves, or emit telemetry. Save files and key configuration remain local.

The source tree and release binaries intentionally contain neither the Xbox nor PC game-layer AES key. Never paste a key or a real save into a public issue.

## Reporting a vulnerability

Open a GitHub security advisory for vulnerabilities that could cause arbitrary file overwrite, ZIP path traversal, key disclosure, or corrupted output being reported as valid. Do not attach real save files or completed key configuration.

## Release binaries

Release artifacts are built by GitHub Actions from a version tag. They are not commercially code-signed or notarized. Verify `SHA256SUMS.txt`, or build from source when provenance is critical.

