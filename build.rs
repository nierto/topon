// g_math 0.7 no longer reads GMATH_PROFILE / GMATH_FRAC_BITS: the format is a
// type. Topon keeps the compile-time selection so one build hashes in one
// format, and maps the same variables to the matching g_math 0.7 type
// (`$OUT_DIR/format.rs`, included as `topon::FixedPoint`).
fn main() {
    println!("cargo:rerun-if-env-changed=GMATH_PROFILE");
    println!("cargo:rerun-if-env-changed=GMATH_FRAC_BITS");

    let profile = std::env::var("GMATH_PROFILE")
        .unwrap_or_else(|_| "embedded".to_string());

    let (table_format, ty) = match profile.to_lowercase().as_str() {
        "realtime" => {
            // As in g_math 0.6, GMATH_FRAC_BITS only splits the realtime word.
            let f: u32 = match std::env::var("GMATH_FRAC_BITS") {
                Ok(s) => s.parse().unwrap_or_else(|_| panic!("GMATH_FRAC_BITS={s:?} is not an integer")),
                Err(_) => 16,
            };
            assert!((2..=30).contains(&f), "GMATH_FRAC_BITS={f} outside 2..=30");
            ("q16_16", format!("Fixed<i32, {f}>"))
        }
        "compact" | "fast" => ("q32_32", "Q32_32".to_string()),
        "embedded" => ("q64_64", "Q64_64".to_string()),
        "balanced" => ("q128_128", "Q128_128".to_string()),
        "scientific" => ("q256_256", "Q256_256".to_string()),
        _ => ("q64_64", "Q64_64".to_string()),
    };

    println!("cargo:rustc-cfg=table_format=\"{}\"", table_format);

    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("format.rs");
    std::fs::write(
        out,
        format!(
            "/// The fixed-point format this build hashes in, chosen by `GMATH_PROFILE`\n\
             /// (and `GMATH_FRAC_BITS` on realtime) at compile time.\n\
             pub type FixedPoint = g_math::fixed_point::{ty};\n"
        ),
    )
    .unwrap();
}
