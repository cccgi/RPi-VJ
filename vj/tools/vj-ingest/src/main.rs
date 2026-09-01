//! CLI binary for vj-ingest: show initialization, automated transcoding, clip ingestion, and Pi 5 transfer.

use std::env;
use vj_ingest::{export_to_usb, ingest_clip, init_show, sync_to_pi};

fn print_usage() {
    println!("vj-ingest: Automated Media Transcoding & Show Ingestion for RPi-VJ\n");
    println!("Usage:");
    println!("  vj-ingest new <show_name> [output_directory]");
    println!("  vj-ingest clip <video_file> <show_directory>");
    println!("  vj-ingest probe <video_file>");
    println!("  vj-ingest export-usb <show_directory> <usb_mount_path>");
    println!("  vj-ingest sync <show_directory> <pi_ssh_target>\n");
    println!("Examples:");
    println!("  vj-ingest new \"Warehouse Set\" ./my_show");
    println!("  vj-ingest clip ./raw_video.mov ./my_show");
    println!("  vj-ingest export-usb ./my_show E:\\");
    println!("  vj-ingest sync ./my_show pi@192.168.1.150");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "new" => {
            let name = args.get(2).cloned().unwrap_or_else(|| "Untitled Show".into());
            let out_dir = args.get(3).cloned().unwrap_or_else(|| "./show".into());
            println!("Initializing new show '{}' in '{}'...", name, out_dir);
            match init_show(&name, &out_dir) {
                Ok(_) => println!("Successfully created show bundle at: {}", out_dir),
                Err(e) => eprintln!("Error creating show: {}", e),
            }
        }
        "clip" => {
            if args.len() < 4 {
                eprintln!("Error: 'clip' command requires <video_file> and <show_directory>");
                return;
            }
            let file = &args[2];
            let show_dir = &args[3];
            println!("Ingesting clip '{}' into show '{}'...", file, show_dir);
            match ingest_clip(file, show_dir, None) {
                Ok(entry) => {
                    println!("Successfully ingested clip:");
                    println!("  ID: {}", entry.id);
                    println!("  Duration: {} ms", entry.duration_ms);
                    println!("  Thumbnail: {}", entry.thumb_path);
                    for (tier, path) in entry.renditions {
                        println!("  Rendition [{:?}]: {}", tier, path);
                    }
                }
                Err(e) => eprintln!("Error ingesting clip: {}", e),
            }
        }
        "probe" => {
            if args.len() < 3 {
                eprintln!("Error: 'probe' requires <video_file>");
                return;
            }
            let file = &args[2];
            match vj_ingest::probe::VideoMetadata::probe_file(file) {
                Ok(meta) => {
                    println!("Probe results for '{}':", file);
                    println!("  Codec: {}", meta.codec_name);
                    println!("  Resolution: {}x{}", meta.width, meta.height);
                    println!("  FPS: {:.2}", meta.fps);
                    println!("  Duration: {:.2}s", meta.duration_secs);
                    println!("  Pixel format: {}", meta.pixel_format);
                    println!("  Pi 5 Hardware Compliant: {}", if meta.is_pi5_compliant() { "YES (Zero-copy HEVC)" } else { "NO (Needs Transcode)" });
                }
                Err(e) => eprintln!("Probe failed: {}", e),
            }
        }
        "export-usb" => {
            if args.len() < 4 {
                eprintln!("Error: 'export-usb' requires <show_directory> and <usb_mount_path>");
                return;
            }
            let show_dir = &args[2];
            let usb_path = &args[3];
            match export_to_usb(show_dir, usb_path) {
                Ok(_) => println!("USB export completed successfully!"),
                Err(e) => eprintln!("USB export failed: {}", e),
            }
        }
        "sync" => {
            if args.len() < 4 {
                eprintln!("Error: 'sync' requires <show_directory> and <pi_ssh_target>");
                return;
            }
            let show_dir = &args[2];
            let target = &args[3];
            match sync_to_pi(show_dir, target) {
                Ok(_) => println!("Sync completed successfully!"),
                Err(e) => eprintln!("Sync failed: {}", e),
            }
        }
        _ => print_usage(),
    }
}
