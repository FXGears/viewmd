fn main() {
    println!("cargo:rustc-link-lib=advapi32");

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
