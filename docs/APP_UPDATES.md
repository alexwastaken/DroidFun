# Desktop app update releases

The desktop updater reads `https://raw.githubusercontent.com/alexwastaken/DroidFun/main/distribution/updater.json`. The feed uses explicit versioned GitHub release URLs and supports published prereleases. It never resolves GitHub's `latest` endpoint.

Supported automatic update packages:

| Feed target | Release asset |
| --- | --- |
| `darwin-aarch64`, `darwin-x86_64` | `DroidFun-App-VERSION-macOS-universal.app.tar.gz` |
| `windows-x86_64` | `DroidFun-App-VERSION-Windows-x64-setup.exe` |
| `windows-aarch64` | `DroidFun-App-VERSION-Windows-ARM64-setup.exe` |

Each updater asset has an adjacent `.sig` release asset. The Mac DMG/ZIP and Linux DEB/RPM downloads remain available; DEB/RPM installation is manual.

## Signing and builds

The application configuration contains the updater public key. Approved release credentials provide `TAURI_SIGNING_PRIVATE_KEY` as a GitHub Actions secret; its password is empty. The private key is injected only into the native packaging step on Mac/Windows. It is never put in source, build arguments, artifacts, evidence, or the feed. Preserve the same signing key for later updates.

`npm run release -- --target universal-apple-darwin --updater` creates and signs the Mac updater archive. Windows uses its native target plus the same `--updater` option. These commands fail before building if the signing key or configured public key is missing. Both release workflows use this option for supported updater targets. Regular `npm run release` packaging explicitly disables updater artifacts, so local builds require no production secret. Linux packaging also disables them.

Tauri CLI 2.12.2 signs the app version in the signature's authenticated trusted comment. The updater should require that signed version. `scripts/updater-assets.cjs` independently checks the Minisign Ed25519 signature over BLAKE2b-512 of the asset, the trusted-comment signature, the key ID, original package filename, and exact app version. Package evidence records those results and SHA-256 for the updater asset and signature. Draft upload tooling rechecks them before uploading. Platform code signing/notarization remains separate from the updater signature.

`node --test scripts/updater-release.test.cjs` exercises signature and metadata tampering, mismatched source/asset hashes, draft rejection, platform completeness, prerelease support, and local builds without credentials. Tests use public RFC/upstream cryptographic test vectors; they do not generate or read a production key.

## Publishing a feed

1. Build the exact application commit on Mac, Windows x64, Windows ARM64, and Linux using the manual workflows. Existing draft-upload mode may collect the installers, updater packages, signatures, and verification records on the matching draft release.
2. Complete platform checks and publish that exact release with its final assets. Keep `target_commitish` set to the full application commit SHA.
3. Put the Mac and both Windows updater files, `.sig` files, and verification JSON records together in one local directory. Run:

   ```sh
   node scripts/generate-updater-manifest.cjs \
     --version 0.4.2 \
     --source FULL_APPLICATION_COMMIT_SHA \
     --packages /absolute/path/to/verified-packages \
     --output distribution/updater.json \
     --notes /absolute/path/to/update-notes.txt
   ```

   `--notes` is optional. The generator uses a read-only GitHub API request through `gh`. It requires the exact release to be published, all three updater package records to match passing tests/native verification and the source commit, and GitHub's published asset names, byte sizes, SHA-256 digests, and download URLs to match the locally reverified files. Missing platforms, draft releases, duplicate assets, changed signatures, and unsigned version metadata fail closed. The two Mac targets share the verified universal archive. Linux targets are omitted.
4. Review and publish the generated `distribution/updater.json` on `main`. The generator only writes the local file; it never commits, pushes, publishes, or dispatches workflows. Update the feed only after the referenced assets are publicly downloadable.

The first updater-enabled app must be installed normally. Later releases can then be offered in that app. Before claiming end-to-end update acceptance, install a signed newer-version package through the app and verify the restarted version and normal USB behavior. Unit tests and artifact verification alone do not establish that acceptance.
