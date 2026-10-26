# Shiply — Cross-Platform Mini Deployment Platform

**Shiply** is a pure-Rust cross-platform desktop deployment platform (`egui` / `eframe`) inspired by Vercel's deployment dashboard and built around the deployment workflow of your existing projects:

- **`ndm-dq-subscription`** (`D:\NTI\Projects\backend\ndm-dq-subscription`)
- **`indy-subscription`** (`D:\NTI\Projects\frontend\indy-subscription`)

## Key Features

1. **Pre-Loaded Project Pipelines (`dev`, `staging`, `production`)**
   - **`ndm-dq-subscription` (Backend)**:
     - Multi-server target (`10.62.70.80`, `10.62.70.81`), SSH user `appndq`, remote path `/data/indy-service/apps/<app_name>`.
     - Pre-flight SSH check $\rightarrow$ local `npm run build` $\rightarrow$ verify `./dist/server.js` $\rightarrow$ remote clean (preserving `node_modules`) $\rightarrow$ stage `.env.<env>` as `.env` & `scp -r` upload (`./dist`, `package.json`, `package-lock.json`, `.env`) $\rightarrow$ remote `npm ci` via corporate HTTP proxy (`http://10.54.18.42:8080`) $\rightarrow$ remote `pm2 restart`.
   - **`indy-subscription` (Frontend)**:
     - Single-server target (`10.62.70.79`), SSH user `appndq`, remote path `/home/appndq/apps/<app_name>`.
     - Pre-flight SSH check $\rightarrow$ local `npm run test` $\rightarrow$ local `npm run build` $\rightarrow$ verify `./dist` $\rightarrow$ remote `mkdir -p` $\rightarrow$ `scp -r ./dist/*` $\rightarrow$ remote `chmod -R a+rX`.

2. **Customizable Deployment Steps**
   - Enable or disable individual steps via checkboxes before deploying (e.g., skip tests or skip `npm ci`).
   - Edit commands, paths, or transfer sources inline.
   - Reorder (`↑` / `↓`), delete, or add new Local Shell and Remote SSH steps.
   - Built-in template placeholders: `{remote_dir}`, `{remote_user}`, `{app_name}`, `{http_proxy}`, `{env_source}`, `{version}`.

3. **Versioning & Git Integration**
   - Auto-detects current version from `package.json` and displays active Git branch & short commit SHA.
   - One-click semantic version bump (`+Patch`, `+Minor`, `+Major`) or custom version string.
   - Optional pre-deploy `package.json` version bump and post-deploy annotated Git tag creation (`git tag -f -a v<version>`).
   - Displays the **Active (Last Succeeded) Version** on each environment card (`DEV`, `STAGING`, `PROD`).

4. **Vercel-Style Deployment History & One-Click Redeploy**
   - Tracks every deployment across all projects with status (`Succeeded`, `Failed`, `Running`), version badge, environment badge, commit SHA, duration, and full step-by-step console output.
   - Click **Redeploy** on any historical deployment to re-run its exact step snapshot and version.
   - All project configurations and deployment histories persist automatically to disk.

5. **Cross-Platform SSH & Native Credential Store**
   - Reads stored credentials directly from the OS credential store (`indyserver` in Windows Credential Manager via Win32 `CredReadW`, macOS Keychain via `security`, Linux Secret Service via `secret-tool`), with automatic fallback to SSH key authentication or manual password entry in the UI.
   - Uses the `shiply` binary itself as a native cross-platform `SSH_ASKPASS` helper (no PowerShell or runtime C# compilation required).

## Installation & Auto-Update (`.exe`)

Run the included installer script to build the release `.exe`, copy it to `./dist/shiply.exe`, install/update `%LOCALAPPDATA%\Programs\Shiply\shiply.exe`, and create **Desktop** and **Start Menu** shortcuts:

```powershell
.\install.ps1
```

Or install/update directly from any compiled `shiply.exe`:

```powershell
.\dist\shiply.exe --install
```

Whenever you build or open a newer version of `shiply.exe`, it automatically updates your installed copy in `%LOCALAPPDATA%\Programs\Shiply\shiply.exe` while preserving all projects and deployment history in `%LOCALAPPDATA%\shiply\shiply_state.json`.

## Running & Testing

```bash
# Run unit & integration tests
cargo test

# Lint with zero warnings
cargo clippy --all-targets -- -D warnings

# Launch the Shiply desktop application
cargo run --release
```
