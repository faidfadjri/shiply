# Shiply — Universal Desktop Mini Deployment Platform

**Shiply** is a fast, lightweight, cross-platform desktop deployment platform built in pure **Rust** (`eframe` / `egui`). It gives developers and teams a clean, Vercel-like desktop workspace to configure multi-environment deployment pipelines, run local builds/tests, upload artifacts via SCP, execute remote SSH commands, track release versions and Git commit changelogs, and perform one-click rollbacks — without needing heavy CI/CD infrastructure.

---

## ✨ Key Features

- **Universal Pipeline Builder**: Configure custom step-by-step deployment pipelines for any stack (Node.js, TypeScript, Rust, Go, Python, static frontends, PM2, systemd, Docker):
  - **Local Command**: Run build, test, or bundle scripts locally (`npm run build`, `cargo build --release`, `pytest`, etc.) with optional non-fatal exit handling.
  - **Verify Local Path**: Guard deployments by ensuring build outputs exist (e.g. `./dist`, `./dist/server.js`) before touching remote servers.
  - **SCP File Transfer**: Upload build directories, wildcards (`./dist/*`), config files, and automatically stage environment-specific files (`.env.dev` / `.env.prod` $\rightarrow$ `.env`) to multiple target servers.
  - **Remote SSH Command**: Execute remote commands across all target servers (`npm ci`, `pm2 restart`, `systemctl restart`, `chmod`, atomic directory cleanup).
- **PowerShell Script (`.ps1`) Auto-Importer**: Point Shiply at an existing `deploy-*.ps1` script (or project folder) to automatically extract target servers, SSH users, remote paths, proxy settings, `.env` sources, and pipeline steps.
- **Custom Environment Levels**: Create, clone, or delete any environment level (`Development`, `Staging`, `Production`, `UAT`, `QA`, `Canary`, `Sandbox`).
- **Git Branch Selector & Commit Changelog**:
  - Switch Git branches directly from the project header before deploying.
  - Automatically records the Git branch, commit hash, and **commit changelog / diff summary** (`git log` & `git diff --stat` since the previous release) on every deployment record.
- **One-Click Rollback & Re-run**:
  - Every deployment stores an immutable snapshot of its version, Git commit, and executed pipeline steps.
  - Roll back any environment to a previous successful release with a single click.
- **Zero-Prompt SSH Authentication**:
  - Works seamlessly with standard **SSH Keys** or **OS Native Credential Stores** (Windows Credential Manager via Win32 `CredReadW`, macOS Keychain, Linux `libsecret`), plus a built-in native `SSH_ASKPASS` handler so you never have to type passwords per server.
- **Portable App Directory & Auto-Discovery**:
  - Starts completely clean on first install.
  - Stores state in `shiply_state.json` directly inside the app directory. Paste `shiply_state.json`, project `.json` files, or `deploy-*.ps1` scripts into the app folder and Shiply automatically detects and loads them.

---

## 🚀 Installation (`Windows/`, `Linux/`, `Mac/`)

Each platform directory in the repository root contains ready-to-use installer files:

| Platform | Directory | Installer Files |
| :--- | :--- | :--- |
| **Windows** | `./Windows/` | `Windows/setup.exe`, `Windows/shiply.exe`, `Windows/install-windows.ps1` |
| **Linux** | `./Linux/` | `Linux/install-linux.sh` |
| **macOS** | `./Mac/` | `Mac/install-mac.command`, `Mac/install-mac.sh` |

### Option 1: Windows Standalone Installer (`Windows/setup.exe`)
1. Open `./Windows/setup.exe` (or download **`setup.exe`** from [GitHub Releases](https://github.com/faidfadjri/shiply/releases)).
2. Double-click **`setup.exe`** to open the **Shiply Setup Wizard**:
   - Installs Shiply to `%LOCALAPPDATA%\Programs\Shiply\shiply.exe` with `logo.ico`.
   - Creates **Desktop** and **Start Menu** shortcuts.
   - **Auto-Update**: Running a newer `setup.exe` automatically upgrades the existing installation in-place while preserving your `shiply_state.json` projects and deployment history.
   - Supports headless/silent installation via CLI:
     ```powershell
     .\Windows\setup.exe --install
     ```

### Option 2: Linux & macOS Installers (`Linux/` & `Mac/`)
- **Linux**:
  ```bash
  bash ./Linux/install-linux.sh
  ```
  Installs the binary to `~/.local/bin/shiply` and registers `~/.local/share/applications/shiply.desktop`.
- **macOS**:
  Double-click `./Mac/install-mac.command` in Finder or run:
  ```bash
  bash ./Mac/install-mac.sh
  ```
  Creates `/Applications/Shiply.app` (or `~/Applications/Shiply.app`).

### Option 3: Build & Install via PowerShell (`install.ps1`)
From the repository root on Windows:
```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```
This compiles the optimized release binaries, populates `./Windows/setup.exe`, `./Windows/shiply.exe`, `./dist/setup.exe`, and installs Shiply to your system.

---

## 📖 Universal Usage Guide

### 1. Adding a Project (Manual or `.ps1` Auto-Import)
On first launch, Shiply starts with a clean workspace:
- **Auto-Import from `.ps1`**: Click **Auto-Import deploy.ps1 Script** (or **+ Add** in the sidebar $\rightarrow$ **Browse .ps1...**). Select any `deploy-dev.ps1` / `deploy-prod.ps1` file. Shiply automatically parses the script (and any sibling `deploy-*.ps1` files in the same folder) to populate the project name, local directory, target servers, and pipeline steps.
- **Create Manually**: Enter your **Project Name**, **Local Project Directory**, and **Description**, then add your custom environments.

### 2. Managing Environments
- Click the **+ Add / Import Environment** card in any project workspace to create a new environment (`dev`, `staging`, `production`, `uat`, `qa`, etc.), clone steps from an existing environment, or import another `.ps1` script.
- Click the **`×`** button on any environment card (or **Delete Environment** in **Environment & SSH Settings**) to remove an environment at any time.

### 3. Template Variables in Pipeline Steps
You can use the following placeholders inside any **Local Command**, **Verify Path**, **SCP Sources**, or **Remote SSH** step:

| Variable | Description | Example |
| :--- | :--- | :--- |
| `{app_name}` | Target application / PM2 / directory name | `my-api-dev` |
| `{remote_dir}` | Remote deployment directory on target servers | `/var/www/my-api-dev` |
| `{remote_user}` | SSH username for target servers | `deploy` |
| `{http_proxy}` | Optional outbound HTTP/HTTPS proxy URL | `http://10.0.0.1:8080` |
| `{env_source}` | Local environment file staged as `.env` | `./.env.dev` |
| `{version}` | Target release version being deployed | `1.4.2` |

### 4. Versioning, Git Branches & Rollbacks
- **Branch Switcher**: Use the **Branch** dropdown in the top-right project header to switch between any local or remote Git branch before deploying.
- **Semantic Version Bumping**: Click `+Patch`, `+Minor`, or `+Major` (or type a custom version). Enable **Bump package.json** to automatically update `"version"` in `package.json` while preserving file formatting, and **Git Tag** to tag successful releases (`vX.Y.Z`).
- **One-Click Rollback**: Click **Rollback** in the Version Control bar or next to any previous successful release in the **Deployment History** table to check out that release's commit, restore its version, and redeploy its exact pipeline snapshot.

### 5. Portable State & Drop-In File Sync
- Click **Open App Folder (JSON / .ps1)** at the bottom of the left sidebar to open Shiply's data directory.
- You can back up or share **`shiply_state.json`**, or paste project `.json` / `deploy-*.ps1` files directly into the folder — Shiply automatically detects and imports them.

### 6. Configuring SSH Credentials
Shiply includes an interactive **How to Setup OS Credentials** guide directly inside the **Environment & SSH Settings** tab, supporting three authentication methods (checked in order):
1. **Manual Password Override**: Enter a password in the **Environment & SSH Settings** tab for one-off deployments.
2. **OS Native Credential Vault**: Store a generic credential in your OS vault matching the environment's **OS Credential Vault Target** (e.g., `prod-ssh-server`):
   - **Windows (GUI)**: Click **Open Windows Credential Manager** inside Shiply (or *Control Panel $\rightarrow$ Credential Manager $\rightarrow$ Windows Credentials $\rightarrow$ Generic Credentials*).
   - **Windows (CLI)**: `cmdkey /generic:"<vault_target>" /user:"<username>" /pass:"YOUR_SSH_PASSWORD"`
   - **macOS**: `security add-generic-password -s "<vault_target>" -a "<username>" -w "<password>"`
   - **Linux**: `secret-tool store --label="Shiply" service "<vault_target>" username "<username>"`
3. **SSH Keys**: If no password is required, Shiply uses standard key-based OpenSSH authentication (`BatchMode=yes`).

---

## 🧪 Development & Testing

Run the full test suite and linter with zero warnings:
```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

## 📄 License
MIT License
