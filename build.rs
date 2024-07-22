use std::{env, path::{Path, PathBuf}, process::Command};

use serde::Deserialize;

#[derive(Deserialize)]
struct ImageSize {
    width: usize,
    height: usize,
}

fn preprocess_image(input: &Path, output_env: &str) {
    let src_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR env var not set"));
    let input = src_dir.join(input);
    let generator_path = src_dir.join("scripts/to-rgb565.py");

    let input_relative = input
        .canonicalize()
        .expect("failed to canonicalize input path")
        .strip_prefix(src_dir.canonicalize().expect("failed to canonicalize src_dir path"))
        .expect("input not relative to source dir")
        .to_owned();

    let output_base = Path::new(&env::var("OUT_DIR").expect("OUT_DIR env var not set"))
        .join("generated")
        .join(input_relative);
    let output_color = output_base.with_extension("rgb565");
    let output_mask = output_base.with_extension("mask");

    let output = Command::new(generator_path)
        .args([
            "--input",
            &input.display().to_string(),
            "--output-color",
            output_color.to_str().expect("path is not valid utf-8"),
            "--output-mask",
            output_mask.to_str().expect("path is not valid utf-8"),
            "--print-size-json",
        ])
        .output()
        .expect("generator exec failed");
    assert!(output.status.success(), "generator failed");
    let output = output.stdout;

    let ImageSize { width, height } = serde_json::from_str::<ImageSize>(
        &String::from_utf8(output).expect("generator output not valid utf-8"),
    )
    .expect("invalid generator output format");

    println!(
        "cargo::rustc-env={output_env}_MASK={mask}",
        mask = output_mask.display()
    );
    println!(
        "cargo::rustc-env={output_env}_COLOR={color}",
        color = output_color.display()
    );
    println!("cargo::rustc-env={output_env}_WIDTH={width}");
    println!("cargo::rustc-env={output_env}_HEIGHT={height}");
}

fn main() {
    preprocess_image(&Path::new("data/dumpster-fire.png"), "DUMPSTER_FIRE");

    // I give up, just comment this out for non-esp builds
    embuild::espidf::sysenv::output();
}
