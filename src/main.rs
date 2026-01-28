use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn is_png(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
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
    list_pb.set_message("Listing png files...");
    list_pb.enable_steady_tick(std::time::Duration::from_millis(80));

    let mut png_files = Vec::new();
    for entry in WalkDir::new(&input_root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if is_png(path) {
                png_files.push(path.to_path_buf());
                if png_files.len() % 100 == 0 {
                    list_pb.set_message(format!("Listing png files... {}", png_files.len()));
                }
            }
        }
    }
    list_pb.finish_with_message(format!("Found {} png files", png_files.len()));

    let convert_pb = ProgressBar::new(png_files.len() as u64);
    convert_pb.set_style(
        ProgressStyle::with_template("[{bar:40}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    convert_pb.set_message("Converting to jpeg...");

    for input_path in png_files {
        let rel = match input_path.strip_prefix(&input_root) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mut output_path = output_root.join(rel);
        output_path.set_extension("jpg");

        if let Some(parent) = output_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                convert_pb.set_message(format!("Failed creating dir: {}", err));
                convert_pb.inc(1);
                continue;
            }
        }

        match image::open(&input_path) {
            Ok(img) => {
                if let Err(err) = img.save_with_format(&output_path, image::ImageFormat::Jpeg) {
                    convert_pb.set_message(format!("Failed saving: {}", err));
                }
            }
            Err(err) => {
                convert_pb.set_message(format!("Failed reading: {}", err));
            }
        }

        convert_pb.inc(1);
    }

    convert_pb.finish_with_message("Done");
}
