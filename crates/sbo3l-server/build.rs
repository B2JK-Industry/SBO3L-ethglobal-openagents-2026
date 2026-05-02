//! R14 P1 — build-time codegen for the gRPC API.
//!
//! Compiles `proto/sbo3l.proto` into Rust bindings and writes them into
//! `OUT_DIR/sbo3l.v1.rs`. The generated code is included from
//! `src/grpc.rs` via `tonic::include_proto!("sbo3l.v1")`.
//!
//! Gated on the `grpc` cargo feature: when the feature is OFF the build
//! script is a no-op, so HTTP-only builds don't need `protoc` available
//! and don't pay the prost compile cost. When ON we use the vendored
//! protoc binary (`protoc-bin-vendored`) so the build works on hosts
//! without `protoc` on `$PATH`.

fn main() {
    // Always tell cargo to track the proto file — even on a no-op
    // build — so that switching the feature on later picks up
    // current contents without a `cargo clean`.
    println!("cargo:rerun-if-changed=../../proto/sbo3l.proto");

    #[cfg(feature = "grpc")]
    compile_grpc();
}

#[cfg(feature = "grpc")]
fn compile_grpc() {
    // Use the vendored protoc binary so this works on hosts without
    // protoc on PATH. The crate exposes a `protoc_bin_path()` helper
    // that returns the absolute path to the bundled binary.
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored: bundled protoc binary not found for this target");
    // `tonic-build` reads the `PROTOC` env var to locate protoc.
    std::env::set_var("PROTOC", protoc);

    let proto_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("proto");
    let proto_file = proto_root.join("sbo3l.proto");

    tonic_build::configure()
        // Generate both server and client. The TS client doesn't
        // need the Rust client, but our integration tests do
        // (they spawn the server then dial it back via tonic's
        // generated channel).
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto_file], &[proto_root])
        .expect("tonic_build: compile_protos failed");
}
