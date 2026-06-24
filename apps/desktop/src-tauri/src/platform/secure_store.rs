use crate::platform::types::{PlatformError, PlatformResult};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static SECURE_STORE_STUB: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub fn secure_store_set_stub(key: &str, value: &str) -> PlatformResult<()> {
    if key.trim().is_empty() {
        return Err(PlatformError::InvalidPath(
            "secure store key cannot be empty".to_string(),
        ));
    }

    let mut store = SECURE_STORE_STUB
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| PlatformError::Unsupported("secure store lock poisoned".to_string()))?;
    store.insert(key.to_string(), value.to_string());
    Ok(())
}

pub fn secure_store_get_stub(key: &str) -> PlatformResult<Option<String>> {
    if key.trim().is_empty() {
        return Err(PlatformError::InvalidPath(
            "secure store key cannot be empty".to_string(),
        ));
    }

    let store = SECURE_STORE_STUB
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| PlatformError::Unsupported("secure store lock poisoned".to_string()))?;
    Ok(store.get(key).cloned())
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn secure_store_set_macos_keychain(key: &str, value: &str) -> PlatformResult<()> {
    secure_store_set_stub(key, value)
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn secure_store_get_macos_keychain(key: &str) -> PlatformResult<Option<String>> {
    secure_store_get_stub(key)
}

#[cfg(all(target_os = "macos", not(test)))]
pub fn secure_store_set_macos_keychain(key: &str, value: &str) -> PlatformResult<()> {
    macos_keychain::set(key, value)
}

#[cfg(all(target_os = "macos", not(test)))]
pub fn secure_store_get_macos_keychain(key: &str) -> PlatformResult<Option<String>> {
    macos_keychain::get(key)
}

#[cfg(all(target_os = "macos", not(test)))]
mod macos_keychain {
    use super::*;
    use std::ffi::c_void;
    use std::ptr;
    use std::slice;

    const KEYCHAIN_SERVICE: &[u8] = b"com.sofvary.desktop.secure-store";
    const ERR_SEC_SUCCESS: i32 = 0;
    const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecKeychainAddGenericPassword(
            keychain: *mut c_void,
            service_name_length: u32,
            service_name: *const i8,
            account_name_length: u32,
            account_name: *const i8,
            password_length: u32,
            password_data: *const c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;

        fn SecKeychainFindGenericPassword(
            keychain: *mut c_void,
            service_name_length: u32,
            service_name: *const i8,
            account_name_length: u32,
            account_name: *const i8,
            password_length: *mut u32,
            password_data: *mut *mut c_void,
            item_ref: *mut *mut c_void,
        ) -> i32;

        fn SecKeychainItemModifyAttributesAndData(
            item_ref: *mut c_void,
            attr_list: *const c_void,
            length: u32,
            data: *const c_void,
        ) -> i32;

        fn SecKeychainItemFreeContent(attr_list: *mut c_void, data: *mut c_void) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    pub fn set(key: &str, value: &str) -> PlatformResult<()> {
        validate_key(key)?;
        let service_len = len_as_u32(KEYCHAIN_SERVICE.len(), "keychain service")?;
        let account = key.as_bytes();
        let account_len = len_as_u32(account.len(), "secure store key")?;
        let password = value.as_bytes();
        let password_len = len_as_u32(password.len(), "secure store value")?;

        let status = unsafe {
            SecKeychainAddGenericPassword(
                ptr::null_mut(),
                service_len,
                KEYCHAIN_SERVICE.as_ptr() as *const i8,
                account_len,
                account.as_ptr() as *const i8,
                password_len,
                password.as_ptr() as *const c_void,
                ptr::null_mut(),
            )
        };

        match status {
            ERR_SEC_SUCCESS => Ok(()),
            ERR_SEC_DUPLICATE_ITEM => update_existing(account, account_len, password, password_len),
            _ => Err(status_error("save macOS keychain item", status)),
        }
    }

    pub fn get(key: &str) -> PlatformResult<Option<String>> {
        validate_key(key)?;
        let service_len = len_as_u32(KEYCHAIN_SERVICE.len(), "keychain service")?;
        let account = key.as_bytes();
        let account_len = len_as_u32(account.len(), "secure store key")?;
        let mut password_len: u32 = 0;
        let mut password_data: *mut c_void = ptr::null_mut();
        let mut item_ref: *mut c_void = ptr::null_mut();

        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null_mut(),
                service_len,
                KEYCHAIN_SERVICE.as_ptr() as *const i8,
                account_len,
                account.as_ptr() as *const i8,
                &mut password_len,
                &mut password_data,
                &mut item_ref,
            )
        };

        if status == ERR_SEC_ITEM_NOT_FOUND {
            return Ok(None);
        }
        if status != ERR_SEC_SUCCESS {
            return Err(status_error("read macOS keychain item", status));
        }

        let bytes = if password_data.is_null() || password_len == 0 {
            Vec::new()
        } else {
            unsafe { slice::from_raw_parts(password_data as *const u8, password_len as usize) }
                .to_vec()
        };

        unsafe {
            if !password_data.is_null() {
                let _ = SecKeychainItemFreeContent(ptr::null_mut(), password_data);
            }
            if !item_ref.is_null() {
                CFRelease(item_ref);
            }
        }

        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| PlatformError::Unsupported("macOS keychain item is not UTF-8".to_string()))
    }

    fn update_existing(
        account: &[u8],
        account_len: u32,
        password: &[u8],
        password_len: u32,
    ) -> PlatformResult<()> {
        let service_len = len_as_u32(KEYCHAIN_SERVICE.len(), "keychain service")?;
        let mut item_ref: *mut c_void = ptr::null_mut();
        let find_status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null_mut(),
                service_len,
                KEYCHAIN_SERVICE.as_ptr() as *const i8,
                account_len,
                account.as_ptr() as *const i8,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut item_ref,
            )
        };
        if find_status != ERR_SEC_SUCCESS {
            return Err(status_error(
                "find existing macOS keychain item",
                find_status,
            ));
        }

        let update_status = unsafe {
            SecKeychainItemModifyAttributesAndData(
                item_ref,
                ptr::null(),
                password_len,
                password.as_ptr() as *const c_void,
            )
        };

        unsafe {
            if !item_ref.is_null() {
                CFRelease(item_ref);
            }
        }

        if update_status == ERR_SEC_SUCCESS {
            Ok(())
        } else {
            Err(status_error("update macOS keychain item", update_status))
        }
    }

    fn validate_key(key: &str) -> PlatformResult<()> {
        if key.trim().is_empty() {
            return Err(PlatformError::InvalidPath(
                "secure store key cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn len_as_u32(len: usize, label: &str) -> PlatformResult<u32> {
        u32::try_from(len).map_err(|_| {
            PlatformError::Unsupported(format!("{label} is too large for macOS keychain"))
        })
    }

    fn status_error(context: &str, status: i32) -> PlatformError {
        PlatformError::Unsupported(format!("{context} failed with OSStatus {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_round_trips_secret_by_reference_key() {
        secure_store_set_stub("sofvary.test.secret", "value").expect("secret should save");
        assert_eq!(
            secure_store_get_stub("sofvary.test.secret").expect("secret should read"),
            Some("value".to_string())
        );
    }

    #[test]
    fn stub_rejects_empty_key() {
        assert!(secure_store_set_stub(" ", "value").is_err());
        assert!(secure_store_get_stub(" ").is_err());
    }
}
