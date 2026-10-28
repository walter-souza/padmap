// No Windows, embute o ícone (padmap.rc) no padmap.exe.
fn main() {
    println!("cargo:rerun-if-changed=padmap.rc");
    println!("cargo:rerun-if-changed=../../assets/padmap.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("padmap.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("não foi possível embutir o ícone");
    }
}
