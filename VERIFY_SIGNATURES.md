# Verifying Release Signatures

All AAEQ release artifacts are signed using [Sigstore cosign](https://github.com/sigstore/cosign) with keyless OIDC signing. This ensures the authenticity and integrity of the downloaded files.

## What Gets Signed

Each release includes signatures for:
- Linux x64 tarball (`.tar.gz`)
- Linux x64 AppImage (`.AppImage`)
- Linux ARM64 tarball (`.tar.gz`)
- macOS DMG (`.dmg`)
- macOS tarball (`.tar.gz`)
- Windows ZIP (`.zip`)
- Windows MSI installer (`.msi`)

Each artifact has two associated files:
- `.sig` - The signature file
- `.pem` - The certificate file

## How to Verify

### Install cosign

First, install cosign on your system:

**Linux:**
```bash
# Download and install from GitHub releases
wget https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64
chmod +x cosign-linux-amd64
sudo mv cosign-linux-amd64 /usr/local/bin/cosign
```

**macOS:**
```bash
brew install cosign
```

**Windows:**
```powershell
# Using winget
winget install sigstore.cosign
```

### Verify a Downloaded File

After downloading a release artifact and its corresponding `.sig` and `.pem` files:

```bash
# Example: Verify the Linux AppImage
cosign verify-blob \
  --certificate aaeq-linux-x64.AppImage.pem \
  --signature aaeq-linux-x64.AppImage.sig \
  --certificate-identity https://github.com/jaschadub/AAEQ/.github/workflows/build.yml@refs/heads/main \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  aaeq-linux-x64.AppImage
```

Replace the filenames with the artifact you downloaded.

### Expected Output

If verification succeeds, you'll see:
```
Verified OK
```

If verification fails, cosign will return an error and the file should not be trusted.

## What Does Keyless Signing Mean?

AAEQ uses Sigstore's keyless signing, which means:
- ✅ No private keys to manage or leak
- ✅ Signatures are tied to the GitHub Actions workflow identity
- ✅ Transparency log provides an immutable record
- ✅ Automatic certificate rotation

The signature proves the artifact was built by the official AAEQ GitHub Actions workflow and hasn't been tampered with since.

## Automated Verification

You can automate verification in your scripts:

```bash
#!/bin/bash
set -e

ARTIFACT="aaeq-linux-x64.AppImage"
CERT_IDENTITY="https://github.com/jaschadub/AAEQ/.github/workflows/build.yml@refs/heads/main"
OIDC_ISSUER="https://token.actions.githubusercontent.com"

# Download artifact and signature files
wget "https://github.com/jaschadub/AAEQ/releases/latest/download/${ARTIFACT}"
wget "https://github.com/jaschadub/AAEQ/releases/latest/download/${ARTIFACT}.sig"
wget "https://github.com/jaschadub/AAEQ/releases/latest/download/${ARTIFACT}.pem"

# Verify
cosign verify-blob \
  --certificate "${ARTIFACT}.pem" \
  --signature "${ARTIFACT}.sig" \
  --certificate-identity "${CERT_IDENTITY}" \
  --certificate-oidc-issuer "${OIDC_ISSUER}" \
  "${ARTIFACT}"

echo "✓ Signature verified successfully!"
chmod +x "${ARTIFACT}"
./"${ARTIFACT}"
```

## Learn More

- [Sigstore Documentation](https://docs.sigstore.dev/)
- [Cosign GitHub Repository](https://github.com/sigstore/cosign)
- [Keyless Signing Explained](https://docs.sigstore.dev/cosign/keyless/)
