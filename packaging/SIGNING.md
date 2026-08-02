# Code signing — how Zymbol releases are signed, and what is still manual

Scope: the Windows installer (`.msi`). Linux packages are not code-signed (distributions
sign repositories, not upstream artifacts) and macOS is deliberately unsigned — see
[macOS](#macos-unsigned-on-purpose) at the end.

Status as of v0.0.8 (2026-08-02): **signing is manual.** The repository has no SignPath
secrets, so `release-windows.yml` stops before signing and publishes nothing. Someone signs
the `.msi` in the SignPath portal and uploads it by hand. This document is the checklist to
make that automatic; every step is done in the SignPath web UI unless stated otherwise.

---

## Why there is no `.pfx` in a GitHub secret

Since 1 June 2023, the CA/Browser Forum requires the private key of an OV/EV code signing
certificate to live in hardware certified FIPS 140-2 level 2 (or equivalent). Certificate
authorities no longer issue anything exportable as `.pfx`/`.p12`, so the classic
"secret + `signtool`" pattern is not available for any certificate issued after that date.
A self-signed certificate would work mechanically and be worthless against SmartScreen.

Every automated option therefore works the same way: the key stays in an HSM and the build
sends it a signing request over an API. SignPath is one such service; Azure Artifact Signing
(formerly Trusted Signing) and DigiCert KeyLocker are the main alternatives.

---

## What the workflow already does

`.github/workflows/release-windows.yml` implements the whole GitHub Actions side. It:

1. builds `zymbol.exe` for `x86_64-pc-windows-msvc`,
2. packages it into `zymbol_lang_vX.Y.Z_x86_64_windows.msi` with WiX,
3. copies it to the fixed name `zymbol-lang.msi` and uploads it as the GitHub artifact
   `windows-msi-unsigned`,
4. **fails** unless `SIGNPATH_API_TOKEN` and `SIGNPATH_ORG_ID` are set,
5. submits that artifact to SignPath and waits for the signed file,
6. verifies with `Get-AuthenticodeSignature` that the file coming back is actually signed,
7. uploads the `.msi` and its `SHA256_x86_64_windows_msi.txt` to the release.

Steps 4-6 exist because of what happened in v0.0.8: the signing step carried
`continue-on-error: true` plus an unsigned fallback, the secrets were missing, and the
release shipped unsigned binaries behind a green checkmark.

The identifiers the workflow sends — change these in SignPath, or in the workflow, so that
they match:

| Input | Value sent | Where it is defined |
| --- | --- | --- |
| `project-slug` | `zymbol-lang` | hard-coded in the workflow |
| `signing-policy-slug` | `release-signing` | hard-coded in the workflow |
| `artifact-configuration-slug` | `windows-installer` | hard-coded in the workflow |
| `organization-id` | `${{ secrets.SIGNPATH_ORG_ID }}` | repository secret |
| `api-token` | `${{ secrets.SIGNPATH_API_TOKEN }}` | repository secret |
| `connector-url` | `https://githubactions.connectors.signpath.io` | action default |

---

## The checklist

### 1. Install the SignPath GitHub App

Install it on the **`zymbol-lang`** GitHub organization and grant it access to the
`interpreter` repository. SignPath uses it to read build metadata and confirm the artifact
came from a workflow run in this repository.

### 2. Add the trusted build system and link it to the project

In SignPath: add the predefined trusted build system **GitHub.com** to the organization,
then link it to the **`zymbol-lang`** project. This is what makes origin verification
possible — without the link, a signing request carries no provable origin and the policy
will reject it.

### 3. Check the project, policy and artifact configuration slugs

Confirm these exist with exactly these names (table above): project `zymbol-lang`, signing
policy `release-signing`, artifact configuration `windows-installer`. A mismatch fails the
run with a SignPath error naming the missing slug.

### 4. Shape the artifact configuration around the ZIP

**This is the step that fails first.** `actions/upload-artifact` wraps whatever it is given
in a ZIP archive, so SignPath receives a ZIP containing `zymbol-lang.msi`, not a bare `.msi`.
The artifact configuration's root element must be `<zip-file>` with the installer nested
inside it, roughly:

```xml
<artifact-configuration xmlns="http://signpath.io/artifact-configuration/v1">
  <zip-file>
    <msi-file path="zymbol-lang.msi">
      <authenticode-sign />
    </msi-file>
  </zip-file>
</artifact-configuration>
```

The workflow's `output-artifact-directory: signed` then receives the signed archive and
`signed/zymbol-lang.msi` is copied back over the versioned name.

### 5. Create a submitter user and its API token

Create (or reuse) a user with the **submitter** role on the `release-signing` policy, then
generate an **API token** for that user. A token whose user is not a submitter on that exact
policy authenticates fine and is refused at submission — an easy hour to lose.

### 6. Set the two repository secrets

Run these on a machine where `gh` is authenticated as the **`zymbol-lang`** account
(`gh auth status` to check; `gh auth switch` if it is on the personal account):

```bash
gh secret set SIGNPATH_API_TOKEN -R zymbol-lang/interpreter
gh secret set SIGNPATH_ORG_ID    -R zymbol-lang/interpreter
gh secret list -R zymbol-lang/interpreter        # both must appear
```

### 7. Make sure the policy does not wait for a human

If `release-signing` requires manual approval, every release will hang: the action runs with
`wait-for-completion: true` and a 600 s timeout, so it will sit there and then fail. Either
configure the policy to sign without approval, or accept that a release needs someone in the
portal within ten minutes.

### 8. Dry-run before a real release

```bash
gh workflow run release-windows.yml -R zymbol-lang/interpreter --ref main -f tag=v0.0.8
```

It uploads to the v0.0.8 release with `--clobber`, replacing the hand-signed `.msi` with the
CI-signed one. Check the run log for `Status: Valid` from the verification step, then confirm
the published file:

```bash
gh release download v0.0.8 -R zymbol-lang/interpreter -p '*.msi' -D /tmp/msicheck
python3 -c "
d=open('/tmp/msicheck/zymbol_lang_v0.0.8_x86_64_windows.msi','rb').read()
print('SIGNED' if 'DigitalSignature'.encode('utf-16-le') in d else 'UNSIGNED')"
```

---

## Signing by hand (today's procedure)

Until the checklist is done, this is the release-day procedure. The workflow will have failed
at step 4 and published nothing.

1. Build the `.msi` — either let the workflow run (it builds before it fails, and the
   unsigned installer is available as the `windows-msi-unsigned` artifact) or build locally.
2. Sign it in the SignPath portal and download the result.
3. Hash it in **binary mode** — `sha256sum -b` reproduces the `hash *file` format git-bash
   produces on the runner, so the checksum files stay consistent across releases:

```bash
cd ~/Descargas
sha256sum -b zymbol_lang_vX.Y.Z_x86_64_windows.msi > SHA256_x86_64_windows_msi.txt
gh release upload vX.Y.Z -R zymbol-lang/interpreter \
  zymbol_lang_vX.Y.Z_x86_64_windows.msi SHA256_x86_64_windows_msi.txt --clobber
```

4. Verify what is actually published, not what you uploaded:

```bash
gh release download vX.Y.Z -R zymbol-lang/interpreter -p '*windows*' -D /tmp/wincheck
cd /tmp/wincheck && sha256sum -c SHA256_x86_64_windows_msi.txt
```

### Checking a signature without Windows

`osslsigncode` is the proper tool. Without it:

- **`.msi`** is an OLE compound file — search the raw bytes for the stream name
  `DigitalSignature` encoded UTF-16LE.
- **PE files** (`.exe`, `.dll`) carry the signature in **entry 4** of the optional header's
  data directory (the Certificate Table). Entry 0 is the Export Table; reading it by mistake
  reports every signed binary as unsigned. The directory starts at `e_lfanew + 24 + 112` for
  PE32+ (`+ 96` for PE32), and entry 4 is 32 bytes further in.

---

## Alternatives, if SignPath ever stops fitting

| Option | Cost | Notes |
| --- | --- | --- |
| **SignPath Foundation** (current) | free for OSS | Certificate belongs to the foundation; project must be accepted. Everything above is already built for it. |
| **Azure Artifact Signing** (ex-Trusted Signing) | ~$9.99/month, 5 000 signatures | Open to individual developers; signs `.exe`, `.msi`, `.dll` and scripts without per-artifact configuration. Requires Microsoft identity verification, which takes days. |
| **Own certificate + cloud HSM** (DigiCert KeyLocker, SSL.com eSigner) | ~$300-600/year | Certificate in the project's own name, independent of any signing service. The expensive, fully-controlled option. |

---

## macOS: unsigned on purpose

The macOS binaries are **not** signed with a Developer ID and **not** notarized. That needs
an Apple Developer account at $99/year, which this project does not spend money on — it is
non-commercial and unfunded. Consequence for users: Gatekeeper blocks the binary on first
run, and it takes a right-click → *Open*, or `xattr -d com.apple.quarantine ./zymbol`, to get
past it. This is a deliberate trade, revisitable only if the project ever gets funding.

The same reasoning is why Windows is signed at all: SignPath's open source program costs
nothing.
