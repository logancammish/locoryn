use {
    std::{
        collections::HashMap,
        env, fs, io,
        ops::Deref,
        path::{Path, PathBuf},
    },
    winresource::WindowsResource,
};

fn copy_dir(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir(&source_path, &target_path)?;
        } else {
            fs::copy(source_path, target_path)?;
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    // Set Windows icon
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile()?;
    }

    // Copy assets folder to output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir).ancestors().nth(3).unwrap();
    let dirs: HashMap<&str, PathBuf> = vec![
        (
            "config",
            Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("config"),
        ),
        (
            "output",
            Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("output"),
        ),
        (
            "assets",
            Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets"),
        ),
    ]
    .into_iter()
    .collect();

    for dir in dirs.iter() {
        if dir.1.deref().exists() {
            let target = target_dir.join(*dir.0);
            if target.exists() {
                fs::remove_dir_all(&target)?;
            }
            copy_dir(dir.1.deref(), &target)?;
        }
    }

    Ok(())
}
