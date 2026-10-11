// Liga o ícone e as informações de versão ao executável. assets\app.res.o é gerado a partir de
// assets\app.rc com: zig rc /:output-format coff /:target x64 /fo app.res.o app.rc
fn main() {
    let res = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("app.res.o");
    println!("cargo:rerun-if-changed={}", res.display());
    if res.exists() {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}
