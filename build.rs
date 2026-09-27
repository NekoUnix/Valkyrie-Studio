fn main() {
    println!("cargo:rerun-if-changed=src/purism.c");
    println!("cargo:rerun-if-changed=third_party/purism_core/PurismCoreBundle.h");
    cc::Build::new()
        .file("src/purism.c")
        .include("third_party/purism_core")
        .std("c11")
        .warnings(false)
        .compile("valkyrie_purism_core");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
}
