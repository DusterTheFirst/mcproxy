use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Get the list of features from the environment
    let features: Vec<String> = env::vars()
        .filter_map(|(key, _)| {
            if key.starts_with("CARGO_FEATURE_") {
                Some(key.trim_start_matches("CARGO_FEATURE_").to_lowercase())
            } else {
                None
            }
        })
        .collect();

    let feature_code = format!(
        "const ENABLED_FEATURES: [&str; {}] = {:?};",
        features.len(),
        features
    );

    // Create the output directory if it doesn't exist
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::create_dir_all(&out_dir).unwrap();

    // Write the generated code to a file
    fs::write(out_dir.join("features.rs"), feature_code).unwrap();

    #[cfg(feature = "metrics")]
    vergen_gix::Emitter::new()
        .add_instructions(
            &vergen_gix::GixBuilder::default()
                .sha(true)
                .branch(true)
                .build()
                .unwrap(),
        )
        .unwrap()
        .emit()
        .expect("Unable to generate build info");
}
