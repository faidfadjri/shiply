use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=logo.ico");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico_path = manifest_dir.join("logo.ico");
    let Ok(ico) = fs::read(&ico_path) else {
        return;
    };
    if ico.len() < 22 {
        return;
    }

    let img_offset = u32::from_le_bytes([ico[18], ico[19], ico[20], ico[21]]) as usize;
    if img_offset >= ico.len() {
        return;
    }
    let img_data = &ico[img_offset..];

    let mut res = Vec::new();

    // 1. 32-byte empty prelude record
    res.extend_from_slice(&0u32.to_le_bytes()); // DataSize = 0
    res.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize = 32
    res.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x00]); // Type = 0
    res.extend_from_slice(&[0xFF, 0xFF, 0x00, 0x00]); // Name = 0
    res.extend_from_slice(&[0u8; 16]);

    // 2. RT_ICON (Type = 3, Name = 1)
    let icon_size = img_data.len() as u32;
    res.extend_from_slice(&icon_size.to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    res.extend_from_slice(&[0xFF, 0xFF, 0x03, 0x00]); // RT_ICON = 3
    res.extend_from_slice(&[0xFF, 0xFF, 0x01, 0x00]); // ID = 1
    res.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    res.extend_from_slice(&0x1010u16.to_le_bytes()); // MemoryFlags
    res.extend_from_slice(&0x0409u16.to_le_bytes()); // LanguageId (en-US)
    res.extend_from_slice(&0u32.to_le_bytes()); // Version
    res.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    res.extend_from_slice(img_data);
    while res.len() % 4 != 0 {
        res.push(0);
    }

    // 3. RT_GROUP_ICON (Type = 14, Name = 1)
    let mut grp_data = Vec::with_capacity(20);
    grp_data.extend_from_slice(&ico[0..18]); // ICONDIR (6) + first 12 bytes of ICONDIRENTRY
    grp_data.extend_from_slice(&1u16.to_le_bytes()); // nID = 1

    res.extend_from_slice(&(grp_data.len() as u32).to_le_bytes());
    res.extend_from_slice(&32u32.to_le_bytes());
    res.extend_from_slice(&[0xFF, 0xFF, 0x0E, 0x00]); // RT_GROUP_ICON = 14
    res.extend_from_slice(&[0xFF, 0xFF, 0x01, 0x00]); // ID = 1
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&0x1030u16.to_le_bytes());
    res.extend_from_slice(&0x0409u16.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&0u32.to_le_bytes());
    res.extend_from_slice(&grp_data);
    while res.len() % 4 != 0 {
        res.push(0);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let res_path = out_dir.join("shiply_icon.res");
    if fs::write(&res_path, &res).is_err() {
        return;
    }

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "gnu" {
        let obj_path = out_dir.join("shiply_icon.o");
        for tool in ["windres", "x86_64-w64-mingw32-windres"] {
            if let Ok(status) = Command::new(tool)
                .args(["-J", "res", "-O", "coff", "-i"])
                .arg(&res_path)
                .arg("-o")
                .arg(&obj_path)
                .status()
            {
                if status.success() && obj_path.exists() {
                    println!("cargo:rustc-link-arg-bins={}", obj_path.display());
                    break;
                }
            }
        }
    } else {
        println!("cargo:rustc-link-arg-bins={}", res_path.display());
    }
}
