fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.set("ProductName", "Harfiy");
        res.set("CompanyName", "Abdumannon Shamsiyev");
        res.set(
            "FileDescription",
            "Harfiy - O'zbek Alifbosi Avto-Konvertatsiya",
        );
        res.set("LegalCopyright", "Copyright (c) 2026 Abdumannon Shamsiyev");

        let manifest_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("harfiy.exe.manifest");
        if manifest_path.exists() {
            res.set_manifest_file(manifest_path.to_str().unwrap());
        } else {
            res.set_manifest_file("harfiy.exe.manifest");
        }

        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to compile windows resource: {}", e);
        }
    }
}
