fn main() {
    let args: Vec<String> = std::env::args().collect();
    let archive = args.get(1).expect("usage: spike-sevenz-rust <path-to-7z>");
    let out_dir = std::env::temp_dir().join("modlab-spike-sevenz-out");
    let _ = std::fs::remove_dir_all(&out_dir);
    sevenz_rust::decompress_file(archive, &out_dir).expect("extraction failed");
    let count = walkdir_count(&out_dir);
    println!("OK: extracted {count} files to {}", out_dir.display());
}

fn walkdir_count(dir: &std::path::Path) -> usize {
    let mut n = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() { n += walkdir_count(&p); } else { n += 1; }
        }
    }
    n
}
