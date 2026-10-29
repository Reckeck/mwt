# Security policy

## Supported versions

Only the latest release (`X.Y.Z` from `main`) receives security fixes. Dev pre-releases (`X.Y.Z-N`
from `development`) are for testing and are fixed by the next build. Please reproduce a problem on the
latest release before reporting it.

## Reporting a vulnerability

Do not open a public issue. Use GitHub private vulnerability reporting:
[Security tab, Report a vulnerability](https://github.com/Reckeck/mwt/security/advisories/new).

Include the version and platform, the steps to reproduce, and what an attacker gains. Do not attach a
real vault or real credentials; a test vault is enough. You will get a reply as soon as the maintainer
can read the report, and a fix and a coordinated disclosure date will be agreed in the advisory.

## What counts as a vulnerability

MWT keeps its data in one local, encrypted vault file. The threat model is in
[docs/security.md](docs/security.md). In scope:

- Reading vault contents without the password, or a way to make the app accept a forged or modified
  vault (a wrong password or a tampered file must fail with an error, never produce data).
- Weakness in the vault crypto: key derivation, nonce handling, the file header, atomic writes.
- The password or the key leaking outside process memory (logs, files, IPC, crash output).
- Remote access: the local-network sync channel ([docs/sync.md](docs/sync.md)) pairing, trust or
  authentication, or a request the app sends that exposes balances or credentials.
- Importing a crafted file (vault, CSV, exported data) that runs code or corrupts the vault.
- Exchange API keys stored in the vault being exposed or used beyond read-only requests.

Out of scope (known limits, see [docs/security.md](docs/security.md)):

- An attacker who already controls the unlocked session or the operating system account while the
  vault is open (the key lives in process memory then).
- Guessing a weak password offline against a stolen vault file: choose a long, unique password.
- No rate limit on unlock attempts inside the app against a local process.
- Lost passwords: there is no recovery by design.
- Unsigned Windows and macOS builds triggering OS warnings.
- Vulnerabilities in third-party price or explorer services themselves.
