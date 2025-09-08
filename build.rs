fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .compile_protos(&["proto/sine.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/sine.proto");
    Ok(())
}