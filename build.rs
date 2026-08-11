fn main() {
    println!("cargo:rerun-if-env-changed=GMATH_PROFILE");

    let profile = std::env::var("GMATH_PROFILE")
        .unwrap_or_else(|_| "embedded".to_string());

    let table_format = match profile.to_lowercase().as_str() {
        "realtime" => "q16_16",
        "compact" | "fast" => "q32_32",
        "embedded" => "q64_64",
        "balanced" => "q128_128",
        "scientific" => "q256_256",
        _ => "q64_64",
    };

    println!("cargo:rustc-cfg=table_format=\"{}\"", table_format);
}
