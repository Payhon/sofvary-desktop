include!(concat!(env!("OUT_DIR"), "/builtin_resource_index.rs"));

pub fn get_builtin_resource(path: &str) -> Option<&'static str> {
    BUILTIN_RESOURCES
        .iter()
        .find(|resource| resource.path == path)
        .map(|resource| resource.contents)
}

pub fn builtin_resource_paths() -> impl Iterator<Item = &'static str> {
    BUILTIN_RESOURCES.iter().map(|resource| resource.path)
}
