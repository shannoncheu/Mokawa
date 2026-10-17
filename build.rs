fn main() {
    #[cfg(windows)]
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        println!("cargo:rerun-if-changed=resources/mokawa.rc");
        println!("cargo:rerun-if-changed=resources/mokawa.ico");
        embed_resource::compile("resources/mokawa.rc", embed_resource::NONE)
            .manifest_required()
            .expect("Mokawa Windows version resource must compile");
    }
}
