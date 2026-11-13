#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("ProductName", "Rustium");
    res.set("FileDescription", "Rustium Web Browser");
    res.set("CompanyName", "Rustium Team");
    res.set("LegalCopyright", "Copyright (c) 2026 Rustium Team");
    res.set("ProductVersion", "1.1.1");
    res.set("FileVersion", "1.1.1");
    if let Err(e) = res.compile() {
        eprintln!("Warning: could not compile Windows resources: {}", e);
    }
}

#[cfg(not(windows))]
fn main() {}
