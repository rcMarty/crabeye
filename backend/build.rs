fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Re-run build script when these change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=migrations");

    Ok(())
}
