use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use walkdir::WalkDir;

const JPG_QUALITY: u8 = 95;

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .filter(|ext| {
            ["jpg", "png", "jpeg"]
                .map(|right| right.eq_ignore_ascii_case(ext))
                .iter()
                .any(|&x| x)
        })
        .is_some()
}

fn write_image(img: image::DynamicImage, output_root: &Path) -> anyhow::Result<()> {
    let writer = File::open(output_root)?;
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, JPG_QUALITY);
    encoder.encode_image(&img)?;
    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);
    let input_root = match args.next() {
        Some(v) => PathBuf::from(v),
        None => {
            eprintln!("Usage: image-parse <input_dir> <output_dir>");
            std::process::exit(1);
        }
    };
    let output_root = match args.next() {
        Some(v) => PathBuf::from(v),
        None => {
            eprintln!("Usage: image-parse <input_dir> <output_dir>");
            std::process::exit(1);
        }
    };

    if !input_root.is_dir() {
        eprintln!("Input path is not a directory: {}", input_root.display());
        std::process::exit(1);
    }

    let list_pb = ProgressBar::new_spinner();
    list_pb.set_message("Listing image files...");
    list_pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let mut png_files = Vec::new();
    let mut png_count = 0usize;
    let mut jpg_count = 0usize;
    let mut jpeg_count = 0usize;
    for entry in WalkDir::new(&input_root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if is_image(path) {
                png_files.push(path.to_path_buf());
                if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                    match ext.to_ascii_lowercase().as_str() {
                        "png" => png_count += 1,
                        "jpg" => jpg_count += 1,
                        "jpeg" => jpeg_count += 1,
                        _ => {}
                    }
                }
                if png_files.len() % 100 == 0 {
                    list_pb.set_message(format!("Listing image files... {}", png_files.len()));
                }
            }
        }
    }
    list_pb.finish_with_message(format!(
        "Found {} PNG, {} JPG/JPEG",
        png_count,
        jpg_count + jpeg_count
    ));

    let convert_pb = ProgressBar::new(png_files.len() as u64);
    convert_pb.set_style(
        ProgressStyle::with_template(
            "{msg:>7} {bar:40.#f92672/237} {pos}/{len} {eta_precise:.cyan}",
        )
        .unwrap()
        .progress_chars("━ ━"),
    );
    convert_pb.set_message("convert");

    let pb = Arc::new(convert_pb);
    let (tx, rx) = crossbeam_channel::unbounded::<PathBuf>();
    let worker_count = num_cpus::get().max(1);
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let rx = rx.clone();
        let pb = Arc::clone(&pb);
        let input_root = input_root.clone();
        let output_root = output_root.clone();
        handles.push(thread::spawn(move || {
            while let Ok(input_path) = rx.recv() {
                let rel = match input_path.strip_prefix(&input_root) {
                    Ok(v) => v,
                    Err(_) => {
                        pb.inc(1);
                        continue;
                    }
                };
                let mut output_path = output_root.join(rel);
                output_path.set_extension("jpg");

                if let Some(parent) = output_path.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        pb.set_message(format!("Failed creating dir: {}", err));
                        pb.inc(1);
                        continue;
                    }
                }

                match image::open(&input_path) {
                    Ok(img) => {
                        if let Err(err) = write_image(img, output_path.as_path()) {
                            pb.set_message(format!("Failed saving: {}", err));
                        }
                    }
                    Err(err) => {
                        pb.set_message(format!("Failed reading: {}", err));
                    }
                }

                pb.inc(1);
            }
        }));
    }

    for input_path in png_files {
        if tx.send(input_path).is_err() {
            break;
        }
    }
    drop(tx);

    for handle in handles {
        let _ = handle.join();
    }

    pb.set_style(
        ProgressStyle::with_template(
            "{msg:>7} {bar:40.#729c1f/237} {pos}/{len} {elapsed_precise:.yellow}",
        )
        .unwrap()
        .progress_chars("━ ━"),
    );
    pb.finish_with_message("Done");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_is_png() {
        assert!(is_image(Path::new("test.png")));
        assert!(is_image(Path::new("test.JPG")));
        assert!(is_image(Path::new("test.JPEG")));
        assert!(!is_image(Path::new("test.txt")));
    }
}
