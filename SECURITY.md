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
- Exchange API keys being exposed (they live only in the device secrets file, never in the vault
  document, sync, backups or the JSON view) or used beyond read-only requests.
- The `dev_*` commands or a decrypted document on disk in a release build (they exist only in
  development builds).

## What the protection does

- Protection levels: no password, a PIN, and a long password (level 3) with a memory-hard key
  derivation whose cost is read from the vault file header. Exchange API keys and crypto data
  require level 3 and never go below it.
- Autolock on idle, sleep and screen lock; it cannot be turned off while the device holds API keys.
- Exchange API keys stay on the device they were entered on, in a separate encrypted file.
- Changing the password re-encrypts the vault and its backup copies next to it.
- Quick unlock (Touch ID on macOS, Windows Hello, the fingerprint on Android) binds the password
  to the device by the OS.
- Not yet checked on a real device: sleep and screen-lock autolock on each OS, Touch ID,
  Windows Hello and the Android fingerprint unlock.

**The honest limit.** If malware runs on a system that is unlocked, or controls your OS account,
this protection only makes attacks harder: the vault key and the decrypted data are in the
memory of the running app and can be read by software with your rights. The protection is
strongest for a stolen or copied file and a locked, powered-off or sleeping device.

Out of scope (known limits, see [docs/security.md](docs/security.md)):

- An attacker who already controls the unlocked session or the operating system account while the
  vault is open (the key lives in process memory then).
- Guessing a weak password offline against a stolen vault file: choose a long, unique password.
- No rate limit on unlock attempts inside the app against a local process.
- Lost passwords: there is no recovery by design.
- Unsigned Windows and macOS builds triggering OS warnings.
- Vulnerabilities in third-party price or explorer services themselves.
