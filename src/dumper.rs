use std::{fs, io};

use chrono::prelude::*;

pub fn dump(filename_root: &str, data: &[u8]) -> io::Result<()> {
    fs::create_dir_all("./dumps")?;
    let now: DateTime<Local> = Local::now();
    let filename = format!(
        "./dumps/{}_{}.bin",
        filename_root,
        now.format("%Y-%m-%d_%H-%M-%S%.3f")
    );
    fs::write(filename, data)
}
