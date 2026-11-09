# Embedded breached-password filter

`breached-passwords.bloom` is a Bloom filter over the SHA-1 of common breached passwords. The
application always consults it when a password is set (BR-2.3); the optional online check
(Have I Been Pwned k-anonymity range API) adds the full corpus when an admin enables it.

## Source
- **List**: `Passwords/Common-Credentials/Pwdb_top-1000000.txt` from
  [danielmiessler/SecLists](https://github.com/danielmiessler/SecLists) (MIT licence, see
  `THIRD-PARTY-NOTICES.md`), at commit `a1a06ab57c3f4efc450d80738a72c11597aac81a` (2026-11-09).
- **Source file SHA-256**: `e9a88f67aafe65496682dc374559ee714e978bee50314767494c3e37a18c9fc8`
  (1,000,000 lines, 8,755,171 bytes).
- **Provenance note**: the repository is MIT-licensed; the list is compiled from public password
  leaks and carries no separate data licence. Only a one-way filter of it is distributed.

## How it was built
```sh
cargo run -p tandem-breach-filter -- \
  --input Pwdb_top-1000000.txt --output crates/credentials/data/breached-passwords.bloom \
  --format plain --min-length 10 --fpr 1e-5
```
- Passwords shorter than 10 characters are omitted: the password policy refuses them anyway (BR-2.1).
- Result: 151,512 entries, 453,849 bytes, target false-positive rate 1 in 100,000 (a good password is
  wrongly refused about once in 100,000 tries; a listed password is never wrongly accepted).

## Replacing it
The decision to use this list instead of the Have I Been Pwned most-prevalent set was taken during
U1 code generation because the full corpus was impractical to download (plan risk R2). To switch,
build a filter from the ordered corpus and replace the file:
```sh
cargo run -p tandem-breach-filter -- --input pwned-passwords-sha1-ordered-by-count.txt \
  --output crates/credentials/data/breached-passwords.bloom --format hash --limit 1000000
```
Attribution for Have I Been Pwned data (CC BY 4.0) must then be added to `THIRD-PARTY-NOTICES.md`.
