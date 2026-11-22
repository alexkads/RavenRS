# ravenrs-secrets

A vault for RavenRS apps and servers. A secret (a token, a password, another service's key) is sealed with ChaCha20-Poly1305 under a key that never sits where the sealed records do. The rest of the program only carries a `SecretRef`, an opaque name, so documents, exports and logs never hold the value.

## What it does

| Piece | What it does |
|---|---|
| `Vault::put` | Seals a value of a kind (`github-token`). Whoever puts it may use, reveal and administer it. |
| `Vault::use_with` | Hands the value to a closure, for whoever may **use** it, without them seeing it. |
| `Vault::reveal` | Gives the value, for whoever may **reveal** it. It is wiped from memory when dropped. |
| `Vault::rotate`, `revoke`, `allow`, `deny` | For whoever **administers** it. Administering does not let anyone see the value, and a secret always keeps someone who administers it. A revoked secret fails its next use with `Revoked`. |
| `SecretRef` | `secret:<uuid>`: what goes in a flow, a pipeline or an export. |
| `KeySource` | Where the key comes from: `KeyFile` (made with 600 if missing, and refused when others can read it), `KeyFromEnv` (64 hexadecimal digits), `FixedKey` for tests, and `SystemKeyring` (Keychain, Credential Manager or Secret Service) with the feature `system-keyring`. |
| `SecretStorage` | Where sealed records go: `MemoryStorage`, or `FileStorage` with one JSON file per secret (600 on Linux and macOS). |
| `Redactor`, `redact` | Keep values out of text, as `•••`. Every value a vault hands out goes into its redactor, rotated ones too. `ravenrs-agent-cli` uses the same `redact`. |

The id, the kind and the version are bound to the sealed value: a record opened with another key, or with any of those changed, fails with `Tampered`.

## Example

```rust
use ravenrs_secrets::{Access, FixedKey, MemoryStorage, Vault};

let vault = Vault::open(&FixedKey([7; 32]), MemoryStorage::new())?;
let token = vault.put("github-token", "ghp_exemplo", "ana")?;
vault.allow(&token, "ana", "agente-ci", Access::Use)?;
let header = vault.use_with(&token, "agente-ci", |value| format!("Authorization: Bearer {value}"))?;
assert_eq!(vault.redactor().redact(&header), "Authorization: Bearer •••");
assert!(vault.reveal(&token, "agente-ci").is_err());
# Ok::<(), ravenrs_secrets::SecretError>(())
```

On a server with no desktop session, keep the key in a file only the service reads (`KeyFile::new("/var/lib/app/vault.key")`) or pass it in the environment (`KeyFromEnv("APP_VAULT_KEY".into())`). Never keep it in the folder of the records.
