fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=proto");
    prost_build::Config::new().compile_protos(&["proto/turnnet/v2/turnnet.proto"], &["proto"])
}
