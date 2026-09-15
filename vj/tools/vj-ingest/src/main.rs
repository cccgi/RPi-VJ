//! CLI binary for vj-ingest: show initialization, automated transcoding, clip ingestion, media analysis, and Pi transfer.

use std::env;
use std::path::Path;
use vj_ingest::analyzer::{analyze_media, QuadrantCrop};
use vj_ingest::priority::apply_low_priority;
use vj_ingest::transcode::{TranscodeOptions, VideoCodec};
use vj_ingest::{export_to_usb, ingest_clip, init_show, sync_to_pi};

fn print_usage() {
    println!("vj-ingest: Automated Media Transcoding & Show Ingestion for RPi-VJ\n");
    println!("Usage:");
    println!("  vj-ingest analyze <video_file> [--json]");
    println!("  vj-ingest transcode <input_file> <output_file> [options]");
    println!("  vj-ingest thumb <video_file> <output_jpg>");
    println!("  vj-ingest clip <video_file> <show_directory>");
    println!("  vj-ingest new <show_name> [output_directory]");
    println!("  vj-ingest export-usb <show_directory> <usb_mount_path>");
    println!("  vj-ingest sync <show_directory> <pi_ssh_target>\n");
    println!("Transcode Options:");
    println!("  --codec <hevc|h264>        (default: hevc)");
    println!("  --crop <tl|tr|bl|br|center>");
    println!("  --crossfade <seconds>      (e.g. 0.5)");
    println!("  --low-priority             (apply nice/ionice background priority)\n");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "analyze" => {
            if args.len() < 3 {
                eprintln!("Error: 'analyze' requires <video_file>");
                return;
            }
            let file = &args[2];
            let as_json = args.iter().any(|a| a == "--json");

            let report = analyze_media(Path::new(file), None);
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                println!("Media Analysis Report for: {}", report.file_path);
                if let Some(m) = &report.metadata {
                    println!("  Codec: {} ({:?})", m.codec_name, m.profile);
                    println!("  Resolution: {}x{}", m.width, m.height);
                    println!("  FPS: {:.2}", m.fps);
                    println!("  Duration: {:.2}s", m.duration_secs);
                    println!("  Pixel Format: {}", m.pixel_format);
                    println!("  Audio Streams: {}", m.audio_streams.len());
                }
                println!("  Decode Valid: {}", report.decode_valid);
                println!(
                    "  Perceptual Hashes: {}",
                    report.perceptual_hashes.join(", ")
                );
                println!(
                    "  Quadrant Scores: avg={:.2}, quad_mirror={}",
                    report.quadrant_scores.avg_quad_similarity,
                    report.quadrant_scores.is_true_quad_mirror
                );
                println!(
                    "  Seam Continuity: {:.2} (seamless={})",
                    report.seam_similarity_score, report.is_seamless_loop
                );
                println!("  Proposed Action: {:?}", report.proposed_action);
                for note in &report.advisory_notes {
                    println!("  Note: {}", note);
                }
            }
        }
        "transcode" => {
            if args.len() < 4 {
                eprintln!("Error: 'transcode' requires <input_file> <output_file>");
                return;
            }
            let in_file = &args[2];
            let out_file = &args[3];

            if args.iter().any(|a| a == "--low-priority") {
                apply_low_priority();
            }

            let mut opts = TranscodeOptions::default();
            for i in 4..args.len() {
                if args[i] == "--codec" && i + 1 < args.len() {
                    match args[i + 1].as_str() {
                        "h264" => opts.codec = VideoCodec::H264,
                        "hevc" => opts.codec = VideoCodec::Hevc,
                        _ => {}
                    }
                }
                if args[i] == "--crop" && i + 1 < args.len() {
                    opts.quadrant_crop = match args[i + 1].as_str() {
                        "tl" | "top-left" => Some(QuadrantCrop::TopLeft),
                        "tr" | "top-right" => Some(QuadrantCrop::TopRight),
                        "bl" | "bottom-left" => Some(QuadrantCrop::BottomLeft),
                        "br" | "bottom-right" => Some(QuadrantCrop::BottomRight),
                        "center" => Some(QuadrantCrop::Center),
                        _ => None,
                    };
                }
                if args[i] == "--crossfade" && i + 1 < args.len() {
                    opts.crossfade_repair = args[i + 1].parse::<f32>().ok();
                }
            }

            println!("Transcoding '{}' -> '{}'...", in_file, out_file);
            match vj_ingest::transcode::transcode_file(in_file, out_file, &opts, None) {
                Ok(_) => println!("Transcode complete!"),
                Err(e) => eprintln!("Transcode failed: {}", e),
            }
        }
        "thumb" => {
            if args.len() < 4 {
                eprintln!("Error: 'thumb' requires <video_file> <output_jpg>");
                return;
            }
            let in_file = &args[2];
            let out_file = &args[3];
            match vj_ingest::thumb::extract_thumbnail(in_file, out_file, 0.5) {
                Ok(_) => println!("Thumbnail extracted to '{}'", out_file),
                Err(e) => eprintln!("Thumbnail extraction failed: {}", e),
            }
        }
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
            match ingest_clip(file, show_dir, None, None) {
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
