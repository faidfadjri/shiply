use std::env;
use std::fs;
use std::io::{self, Write};

pub const ENV_ASKPASS_MODE: &str = "SHIPLY_ASKPASS_MODE";
pub const ENV_ASKPASS_FILE: &str = "SHIPLY_ASKPASS_FILE";
pub const ENV_SSH_PASSWORD: &str = "SHIPLY_SSH_PASSWORD";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCredentials {
    pub username: String,
    pub password: Option<String>,
    pub source_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultCredential {
    pub username: Option<String>,
    pub password: String,
}

/// If the current process was launched by `ssh`/`scp` as `SSH_ASKPASS`,
/// prints the password to stdout and returns `true`.
pub fn try_handle_askpass_invocation() -> bool {
    if env::var_os(ENV_ASKPASS_MODE).is_none() {
        return false;
    }

    if let Some(secret) = read_askpass_secret_from_env() {
        let mut out = io::stdout().lock();
        let _ = out.write_all(secret.as_bytes());
        let _ = out.flush();
    }
    true
}

pub fn read_askpass_secret_from_env() -> Option<String> {
    if let Ok(path) = env::var(ENV_ASKPASS_FILE) {
        if !path.is_empty() {
            if let Ok(contents) = fs::read_to_string(&path) {
                return Some(contents);
            }
        }
    }
    if let Ok(pw) = env::var(ENV_SSH_PASSWORD) {
        if !pw.is_empty() {
            return Some(pw);
        }
    }
    None
}

/// Decodes a Windows Credential Manager `CredentialBlob` which may be stored
/// either as UTF-16LE (standard Windows Generic Credentials) or UTF-8.
pub fn decode_credential_blob(blob: &[u8]) -> String {
    if blob.is_empty() {
        return String::new();
    }

    if blob.len().is_multiple_of(2) && (blob.contains(&0) || std::str::from_utf8(blob).is_err()) {
        let units: Vec<u16> = blob
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&chunk| u16::from_le_bytes(chunk))
            .collect();
        return String::from_utf16_lossy(&units)
            .trim_end_matches('\0')
            .to_string();
    }

    String::from_utf8_lossy(blob)
        .trim_end_matches('\0')
        .to_string()
}

/// Reads a stored credential from the OS native credential store:
/// - Windows: Windows Credential Manager (`CredReadW` Generic Credential, e.g., `indyserver`)
/// - macOS: Keychain (`security find-generic-password`)
/// - Linux: Secret Service (`secret-tool lookup`)
pub fn read_os_vault_credential(target: &str) -> Option<VaultCredential> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }

    #[cfg(windows)]
    {
        read_windows_generic_credential(target)
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args(["find-generic-password", "-s", target, "-w"])
            .output()
            .ok()?;
        if output.status.success() {
            let pw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pw.is_empty() {
                return Some(VaultCredential {
                    username: None,
                    password: pw,
                });
            }
        }
        None
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let output = std::process::Command::new("secret-tool")
            .args(["lookup", "service", target])
            .output()
            .ok()?;
        if output.status.success() {
            let pw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pw.is_empty() {
                return Some(VaultCredential {
                    username: None,
                    password: pw,
                });
            }
        }
        None
    }
}

#[cfg(windows)]
fn read_windows_generic_credential(target: &str) -> Option<VaultCredential> {
    use std::ptr;
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };

    let mut wide_target: Vec<u16> = target.encode_utf16().collect();
    wide_target.push(0);

    let mut cred_ptr: *mut CREDENTIALW = ptr::null_mut();
    // SAFETY: `wide_target` is a valid null-terminated UTF-16 string and `cred_ptr` receives the allocated CREDENTIALW pointer.
    let ok = unsafe {
        CredReadW(
            wide_target.as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut cred_ptr,
        )
    };

    if ok == 0 || cred_ptr.is_null() {
        return None;
    }

    // SAFETY: `cred_ptr` was allocated by `CredReadW` and is freed via `CredFree` before returning.
    unsafe {
        let cred = &*cred_ptr;
        let username = if !cred.UserName.is_null() {
            let mut len = 0usize;
            while *cred.UserName.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(cred.UserName, len);
            let u = String::from_utf16_lossy(slice).trim().to_string();
            if u.is_empty() {
                None
            } else {
                Some(u)
            }
        } else {
            None
        };

        let password = if !cred.CredentialBlob.is_null() && cred.CredentialBlobSize > 0 {
            let blob = std::slice::from_raw_parts(
                cred.CredentialBlob,
                cred.CredentialBlobSize as usize,
            );
            decode_credential_blob(blob)
        } else {
            String::new()
        };

        CredFree(cred_ptr as *const _);

        if password.is_empty() {
            None
        } else {
            Some(VaultCredential { username, password })
        }
    }
}

/// Resolves the SSH username and password by checking:
/// 1. Explicit UI password override (if entered by user)
/// 2. OS Vault / Credential Manager (`vault_target`, e.g., `indyserver`)
/// 3. Fallback to configured username with SSH Key authentication (`password: None`)
pub fn resolve_credentials(
    configured_user: &str,
    vault_target: &str,
    manual_password_override: Option<&str>,
) -> ResolvedCredentials {
    let default_user = if configured_user.trim().is_empty() {
        "appndq".to_string()
    } else {
        configured_user.trim().to_string()
    };

    if let Some(pw) = manual_password_override {
        if !pw.is_empty() {
            return ResolvedCredentials {
                username: default_user,
                password: Some(pw.to_string()),
                source_description: "Manual UI Password".to_string(),
            };
        }
    }

    if let Some(vault_cred) = read_os_vault_credential(vault_target) {
        let username = vault_cred.username.unwrap_or(default_user);
        return ResolvedCredentials {
            username: username.clone(),
            password: Some(vault_cred.password),
            source_description: format!(
                "OS Credential Store ('{vault_target}' as '{username}')"
            ),
        };
    }

    ResolvedCredentials {
        username: default_user,
        password: None,
        source_description: "SSH Key / Agent (No stored password found)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_decode_utf16le_credential_blob() {
        let original = "MySecretP@ss123!";
        let utf16_bytes: Vec<u8> = original
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        assert_eq!(decode_credential_blob(&utf16_bytes), original);
    }

    #[test]
    fn test_decode_utf8_credential_blob() {
        let original = "Utf8SecretPass99";
        assert_eq!(decode_credential_blob(original.as_bytes()), original);
    }

    #[test]
    fn test_resolve_credentials_with_manual_override() {
        let resolved = resolve_credentials("appndq", "nonexistent_vault_xyz", Some("secret123"));
        assert_eq!(resolved.username, "appndq");
        assert_eq!(resolved.password.as_deref(), Some("secret123"));
        assert_eq!(resolved.source_description, "Manual UI Password");
    }

    #[test]
    fn test_read_askpass_secret_from_file() {
        let mut tmp = NamedTempFile::new().expect("temp file");
        write!(tmp, "s3cr3t_pw").expect("write pw");
        env::set_var(ENV_ASKPASS_FILE, tmp.path().as_os_str());
        let read_pw = read_askpass_secret_from_env();
        env::remove_var(ENV_ASKPASS_FILE);
        assert_eq!(read_pw.as_deref(), Some("s3cr3t_pw"));
    }
}
