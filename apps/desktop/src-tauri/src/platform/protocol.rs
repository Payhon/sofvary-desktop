use crate::platform::types::{PlatformError, PlatformResult};

pub fn register_protocol_handler_stub(protocol: &str) -> PlatformResult<()> {
    if protocol.trim().is_empty() {
        return Err(PlatformError::InvalidPath(
            "protocol cannot be empty".to_string(),
        ));
    }

    Ok(())
}
