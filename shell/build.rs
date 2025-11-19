use std::{env, path::Path, process::Command};

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir_path = Path::new(&out_dir);

    // TODO parse package/bin name from Cargo.toml
    let bin_name = env::var("CARGO_BIN_NAME").unwrap_or("tinyShell".into());

    let bin_dir = out_dir_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    if !Command::new("ln")
        .args([
            "-sf",
            &format!("{}", bin_dir.join(&bin_name).display()),
            &format!("{}", manifest_dir.join("a.out").display()),
        ])
        .status()
        .unwrap()
        .success()
    {
        panic!("could not generate symlink a.out to bin {}", &bin_name);
    }
}
