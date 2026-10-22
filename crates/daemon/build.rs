fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.set("ProductName", "Harfiy");
        res.set("CompanyName", "Abdumannon Shamsiyev");
        res.set("FileDescription", "Harfiy - O'zbek Alifbosi Avto-Konvertatsiya");
        res.set("LegalCopyright", "Copyright (c) 2026 Abdumannon Shamsiyev");
        if let Err(e) = res.compile() {
            eprintln!("Warning: failed to compile windows resource: {}", e);
        }
    }
}
