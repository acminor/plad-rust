extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("protos/predictor.capnp")
        .run()
        .unwrap();
}
