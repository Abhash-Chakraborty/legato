# Verify a download

Legato release artifacts are signed with Tauri's updater signature format. The public key is stored in [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) under `plugins.updater.pubkey`.

To verify a release manually, set `$Artifact` to the filename you downloaded, save the `pubkey` value from `src-tauri/tauri.conf.json` to `legato.pub.b64`, then decode the public key and matching `.sig` file from base64 and verify the artifact with `minisign`. In PowerShell:

```powershell
# Replace with the file you downloaded
$Artifact = "Legato_<version>_x64-setup.exe"

$pub = (Get-Content legato.pub.b64 -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\legato.pub", [Convert]::FromBase64String($pub))

$sig = (Get-Content "$Artifact.sig" -Raw).Trim()
[IO.File]::WriteAllBytes("$PWD\$Artifact.minisig", [Convert]::FromBase64String($sig))

minisign -Vm $Artifact -p legato.pub -x "$Artifact.minisig"
```

On success, `minisign` prints:

```text
Signature and comment signature verified
```

Do not use `gpg` for these `.sig` files.
