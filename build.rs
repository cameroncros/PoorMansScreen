fn main() {
    println!("asdfasdf");
    prost_build::compile_protos(&["src/messages.proto"],
                                &["src/"]).unwrap();
    println!("asdfasdf");
    // prost::new()
    //     .pure()
    //     .out_dir("src/")
    //     .include("src")
    //     .input("src/messages.proto")
    //     .run()
    //     .unwrap();
}
