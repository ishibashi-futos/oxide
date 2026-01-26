fn main() {
    if let Ok(value) = std::env::var("OX_BUILD_VERSION") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            println!("cargo:rustc-env=OX_BUILD_VERSION={trimmed}");
        }
    }
}
