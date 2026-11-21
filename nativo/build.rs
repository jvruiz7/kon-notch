// Ícone e dados do Kon.exe no Windows (Explorer, Gerenciador de Tarefas, atalho).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("assets/icon.ico");
        r.set("ProductName", "Kon");
        r.set("FileDescription", "Kon");
        r.set("CompanyName", "jvruiz7");
        if let Err(e) = r.compile() { println!("cargo:warning=sem ícone no exe: {e}"); }
    }
}
