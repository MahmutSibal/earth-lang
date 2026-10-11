//! Keeps the GitHub token that the assistant uses. On Windows it lives in
//! the Credential Manager (encrypted for the signed-in Windows user); on
//! other systems in a file only the user can read, next to the settings.

use crate::auth::Token;

const TARGET: &str = "Earth Studio/GitHub";

pub fn load() -> Option<Token> {
    Token::from_json(&platform::read(TARGET)?)
}

pub fn save(token: &Token) {
    platform::write(TARGET, &token.to_json());
}

pub fn delete() {
    platform::delete(TARGET);
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

    fn file() -> Option<PathBuf> {
        Some(crate::settings::folder()?.join("github-token.json"))
    }

    pub fn write(_target: &str, secret: &str) {
        let Some(path) = file() else {
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

    pub fn read(_target: &str) -> Option<String> {
        fs::read_to_string(file()?).ok()
    }

    pub fn delete(_target: &str) {
        if let Some(path) = file() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    /// Uses its own target, so it leaves the real token alone. On Linux the
    /// target is ignored, so the test only runs where it cannot clash.
    #[test]
    #[cfg(windows)]
    fn stores_reads_and_deletes_a_secret() {
        use super::platform;
        let target = "Earth Studio/test";
        platform::write(target, "ghu_secret");
        assert_eq!(platform::read(target).as_deref(), Some("ghu_secret"));
        platform::delete(target);
        assert_eq!(platform::read(target), None);
    }

    #[test]
    fn token_survives_json() {
        let token = crate::auth::Token {
            access: "ghu_a".to_owned(),
            refresh: Some("ghr_b".to_owned()),
            expires_at: Some(1_800_000_000),
        };
        assert_eq!(crate::auth::Token::from_json(&token.to_json()), Some(token));
    }
}
