fn main() {
    let sha = std::env::var("LAYE_COMMIT_SHA").unwrap_or_else(|_| "dev".to_string());
    let built_at = std::env::var("LAYE_BUILT_AT").unwrap_or_else(|_| "local".to_string());
    println!("cargo:rustc-env=LAYE_COMMIT_SHA={sha}");
    println!("cargo:rustc-env=LAYE_BUILT_AT={built_at}");
    println!("cargo:rerun-if-env-changed=LAYE_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=LAYE_BUILT_AT");
}
