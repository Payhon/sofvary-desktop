use std::env;

const DEFAULT_SOFVARY_ORIGIN: &str = "https://sofvary.vercel.app";
const SOFVARY_BASE_URL_ENV: &str = "SOFVARY_BASE_URL";
const SOFVARY_API_BASE_URL_ENV: &str = "SOFVARY_API_BASE_URL";
const SOFVARY_REGISTRY_BASE_URL_ENV: &str = "SOFVARY_REGISTRY_BASE_URL";
const SOFVARY_WEB_BASE_URL_ENV: &str = "SOFVARY_WEB_BASE_URL";

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn configured_base_url(env_names: &[&str]) -> String {
    for env_name in env_names {
        if let Ok(value) = env::var(env_name) {
            let normalized = normalize_base_url(&value);
            if !normalized.is_empty() {
                return normalized;
            }
        }
    }
    DEFAULT_SOFVARY_ORIGIN.to_string()
}

pub fn sofvary_api_base_url_from_env() -> String {
    configured_base_url(&[
        SOFVARY_API_BASE_URL_ENV,
        SOFVARY_REGISTRY_BASE_URL_ENV,
        SOFVARY_BASE_URL_ENV,
    ])
}

pub fn sofvary_web_base_url_from_env() -> String {
    configured_base_url(&[SOFVARY_WEB_BASE_URL_ENV, SOFVARY_BASE_URL_ENV])
}
