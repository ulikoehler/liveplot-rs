fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Only build the proto files when the 'grpc' feature is enabled
    let grpc_enabled = std::env::var("CARGO_FEATURE_GRPC").is_ok();
    if grpc_enabled {
        tonic_build::configure()
            .build_server(true)
            .compile_protos(&["proto/sine.proto"], &["proto"])?;
        println!("cargo:rerun-if-changed=proto/sine.proto");
    }
    Ok(())
}
