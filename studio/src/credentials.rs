//! Keeps the assistant's API keys, one per AI service. On Windows they
//! live in the Credential Manager (encrypted for the signed-in Windows
//! user); on other systems in files only the user can read, next to the
//! settings.

fn target(service: &str) -> String {
    format!("Earth Studio/AI/{service}")
}

pub fn load_api_key(service: &str) -> Option<String> {
    platform::read(&target(service)).filter(|key| !key.is_empty())
}

pub fn save_api_key(service: &str, key: &str) {
    platform::write(&target(service), key);
}

pub fn delete_api_key(service: &str) {
    platform::delete(&target(service));
}

/// Versions 0.9.x kept a GitHub token for GitHub Models, which GitHub has
/// retired; it is no longer needed.
pub fn forget_github_token() {
    platform::delete("Earth Studio/GitHub");
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    use std::ptr;

    const CRED_TYPE_GENERIC: u32 = 1;
    const CRED_PERSIST_LOCAL_MACHINE: u32 = 2;

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[repr(C)]
    struct Credential {
        flags: u32,
        kind: u32,
        target_name: *mut u16,
        comment: *mut u16,
        last_written: FileTime,
        blob_size: u32,
        blob: *mut u8,
        persist: u32,
        attribute_count: u32,
        attributes: *mut c_void,
        target_alias: *mut u16,
        user_name: *mut u16,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn CredWriteW(credential: *const Credential, flags: u32) -> i32;
        fn CredReadW(
            target: *const u16,
            kind: u32,
            flags: u32,
            credential: *mut *mut Credential,
        ) -> i32;
        fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
        fn CredFree(buffer: *mut c_void);
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub fn write(target: &str, secret: &str) {
        let mut target = wide(target);
        let mut user = wide("Earth Studio");
        let mut blob = secret.as_bytes().to_vec();
        let credential = Credential {
            flags: 0,
            kind: CRED_TYPE_GENERIC,
            target_name: target.as_mut_ptr(),
            comment: ptr::null_mut(),
            last_written: FileTime { low: 0, high: 0 },
            blob_size: blob.len() as u32,
            blob: blob.as_mut_ptr(),
            persist: CRED_PERSIST_LOCAL_MACHINE,
            attribute_count: 0,
            attributes: ptr::null_mut(),
            target_alias: ptr::null_mut(),
            user_name: user.as_mut_ptr(),
        };
        // SAFETY: every pointer refers to a buffer that outlives the call.
        unsafe {
            CredWriteW(&credential, 0);
        }
    }

    pub fn read(target: &str) -> Option<String> {
        let target = wide(target);
        let mut credential: *mut Credential = ptr::null_mut();
        // SAFETY: on success Windows allocates `credential`, which is read
        // and then released with CredFree.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0 {
                return None;
            }
            let blob =
                std::slice::from_raw_parts((*credential).blob, (*credential).blob_size as usize);
            let secret = String::from_utf8(blob.to_vec()).ok();
            CredFree(credential.cast());
            secret
        }
    }

    pub fn delete(target: &str) {
        let target = wide(target);
        // SAFETY: `target` is a valid, NUL-terminated wide string.
        unsafe {
            CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0);
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    /// One file per target, named after it.
    fn file(target: &str) -> Option<PathBuf> {
        let name: String = target
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        Some(crate::settings::folder()?.join("secrets").join(name))
    }

    pub fn write(target: &str, secret: &str) {
        let Some(path) = file(target) else {
            return;
        };
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        if let Ok(mut out) = options.open(path) {
            let _ = out.write_all(secret.as_bytes());
        }
    }

    pub fn read(target: &str) -> Option<String> {
        fs::read_to_string(file(target)?).ok()
    }

    pub fn delete(target: &str) {
        if let Some(path) = file(target) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::platform;

    /// Uses its own target, so the real keys are left alone.
    #[test]
    fn stores_reads_and_deletes_a_secret() {
        let target = "Earth Studio/test";
        #[cfg(not(windows))]
        std::env::set_var(
            "XDG_CONFIG_HOME",
            std::env::temp_dir().join(format!("earth-credentials-{}", std::process::id())),
        );
        platform::write(target, "sk-secret");
        assert_eq!(platform::read(target).as_deref(), Some("sk-secret"));
        platform::delete(target);
        assert_eq!(platform::read(target), None);
    }
}
