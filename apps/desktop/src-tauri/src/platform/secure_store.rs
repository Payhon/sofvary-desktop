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
