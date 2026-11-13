// Embeds the icon on Windows targets; a no-op elsewhere.
fn main() {
    println!("cargo:rerun-if-changed=nitrate.rc");
    println!("cargo:rerun-if-changed=../../assets/logo/nitrate.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Err(e) = embed_resource::compile("nitrate.rc", embed_resource::NONE).manifest_optional()
    {
        // A missing resource compiler only costs the exe icon, never the build.
        println!("cargo:warning=exe icon not embedded: {e}");
    }
}
