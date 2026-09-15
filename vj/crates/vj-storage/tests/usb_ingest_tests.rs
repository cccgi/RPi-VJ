use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use vj_storage::disk::MockDiskSpaceChecker;
use vj_storage::usb::{
    CandidateFile, FileIngestOutcome, InboxScanner, MockMountProvider, MountProvider, UsbCopyEngine,
};
use vj_storage::{JobState, ShowStorageManager, StorageConfig, StorageError};

fn create_sample_file(path: &PathBuf, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut file = File::create(path).unwrap();
    file.write_all(content).unwrap();
    file.sync_all().unwrap();
}

fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

#[test]
fn test_normal_usb_media_copy() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config.clone(), checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr.clone(), mount_provider.clone());

    // Create USB structure with /RPI-VJ-INBOX/
    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    let clip1_content = b"VIDEO_CONTENT_CLIP_1_MP4_SAMPLE_DATA_STREAM";
    let clip2_content = b"ANOTHER_VIDEO_CLIP_2_MOV_SAMPLE_DATA_STREAM";

    let clip1_path = inbox_dir.join("clip1.mp4");
    let clip2_path = inbox_dir.join("subfolder").join("clip2.mov");

    create_sample_file(&clip1_path, clip1_content);
    create_sample_file(&clip2_path, clip2_content);

    // Mount USB using mock mount provider
    mount_provider
        .mount(usb_root, usb_root, Some("vfat"))
        .unwrap();
    assert!(mount_provider.is_mounted(usb_root));

    // Process mount
    let outcomes = engine.process_mount(usb_root).unwrap();
    assert_eq!(outcomes.len(), 2);

    // After processing, drive is safely unmounted
    assert!(!mount_provider.is_mounted(usb_root));

    for outcome in &outcomes {
        match outcome {
            FileIngestOutcome::Copied(job) => {
                assert_eq!(job.state, JobState::Probing);
                assert_eq!(job.progress_percent, 100.0);
                assert!(job.staging_path.exists());

                let sha256_recorded = job.details.get("sha256").unwrap();
                let staged_file = PathBuf::from(job.details.get("staged_file").unwrap());
                assert!(staged_file.exists());

                let staged_content = fs::read(&staged_file).unwrap();
                let actual_hash = compute_sha256(&staged_content);
                assert_eq!(sha256_recorded, &actual_hash);

                if job.source_path == clip1_path {
                    assert_eq!(&staged_content[..], clip1_content);
                    assert_eq!(&actual_hash, &compute_sha256(clip1_content));
                } else if job.source_path == clip2_path {
                    assert_eq!(&staged_content[..], clip2_content);
                    assert_eq!(&actual_hash, &compute_sha256(clip2_content));
                }
            }
            _ => panic!("Expected FileIngestOutcome::Copied, got {:?}", outcome),
        }
    }

    // Verify jobs in JobStore
    let jobs = mgr.job_store().list_jobs().unwrap();
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().all(|j| j.state == JobState::Probing));
}

#[test]
fn test_read_only_media_copy() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config, checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr, mount_provider);

    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    let clip_path = inbox_dir.join("readonly_clip.mp4");
    let content = b"READ_ONLY_VIDEO_DATA_BUFFER";
    create_sample_file(&clip_path, content);

    // Make the source file read-only on disk
    let mut perms = fs::metadata(&clip_path).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&clip_path, perms).unwrap();

    let outcomes = engine.process_mount(usb_root).unwrap();
    assert_eq!(outcomes.len(), 1);

    match &outcomes[0] {
        FileIngestOutcome::Copied(job) => {
            assert_eq!(job.state, JobState::Probing);
            let staged_file = PathBuf::from(job.details.get("staged_file").unwrap());
            assert_eq!(fs::read(&staged_file).unwrap(), content);
        }
        _ => panic!("Expected Copied outcome for read-only media"),
    }
}

#[test]
fn test_duplicate_media_detection_and_reinsertion() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config, checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr.clone(), mount_provider);

    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    let content = b"UNIQUE_IDENTICAL_VIDEO_CONTENT";
    let file1_path = inbox_dir.join("original.mp4");
    let file2_path = inbox_dir.join("copy_under_different_name.mp4");

    create_sample_file(&file1_path, content);
    create_sample_file(&file2_path, content);

    // Ingest candidate 1
    let scan = InboxScanner::scan_inbox(usb_root).unwrap();
    assert_eq!(scan.candidates.len(), 2);

    let cand1 = scan
        .candidates
        .iter()
        .find(|c| c.source_path == file1_path)
        .unwrap();
    let outcome1 = engine.copy_candidate(cand1, || false).unwrap();

    let job1_id = match outcome1 {
        FileIngestOutcome::Copied(job) => {
            // Simulate complete workflow through to Complete state following valid transitions:
            // Probing -> Transcoding -> Validating -> Ready -> Activating -> Complete
            let job = mgr.job_store().transition_job(job.id, |j| j.state = JobState::Transcoding).unwrap();
            let job = mgr.job_store().transition_job(job.id, |j| j.state = JobState::Validating).unwrap();
            let job = mgr.job_store().transition_job(job.id, |j| j.state = JobState::Ready).unwrap();
            let job = mgr.job_store().transition_job(job.id, |j| j.state = JobState::Activating).unwrap();
            let completed_job = mgr.job_store().transition_job(job.id, |j| j.state = JobState::Complete).unwrap();
            completed_job.id
        }
        _ => panic!("Expected first file to be copied"),
    };

    // Ingest candidate 2 with identical content -> should detect duplicate SHA-256
    let cand2 = scan
        .candidates
        .iter()
        .find(|c| c.source_path == file2_path)
        .unwrap();
    let outcome2 = engine.copy_candidate(cand2, || false).unwrap();

    match outcome2 {
        FileIngestOutcome::Duplicate {
            job_id,
            sha256,
            source_path,
        } => {
            assert_eq!(job_id, job1_id);
            assert_eq!(sha256, compute_sha256(content));
            assert_eq!(source_path, file2_path);
        }
        _ => panic!("Expected Duplicate outcome, got {:?}", outcome2),
    }

    // Reinsertion test: reinserting the drive and processing candidate 1 should return SkippedAlreadyProcessed
    let outcome_reinsert = engine.copy_candidate(cand1, || false).unwrap();
    match outcome_reinsert {
        FileIngestOutcome::SkippedAlreadyProcessed(existing_job) => {
            assert_eq!(existing_job.id, job1_id);
            assert_eq!(existing_job.state, JobState::Complete);
        }
        _ => panic!(
            "Expected SkippedAlreadyProcessed on reinsert, got {:?}",
            outcome_reinsert
        ),
    }
}

#[test]
fn test_full_disk_reserve_rejection() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    // Total 100 GB, Free 15 GB (exactly 15% reserve, 0 bytes usable margin)
    let checker = Arc::new(MockDiskSpaceChecker::new(
        15 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config, checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr, mount_provider);

    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    let clip_path = inbox_dir.join("large_video.mp4");
    create_sample_file(&clip_path, b"SOME_BYTES");

    let err = engine.process_mount(usb_root).unwrap_err();
    match err {
        StorageError::InsufficientSpace {
            required_bytes: _,
            free_bytes,
            total_bytes,
            reserve_percent,
        } => {
            assert_eq!(free_bytes, 15 * 1024 * 1024 * 1024);
            assert_eq!(total_bytes, 100 * 1024 * 1024 * 1024);
            assert_eq!(reserve_percent, 15.0);
        }
        _ => panic!("Expected InsufficientSpace error, got {:?}", err),
    }
}

#[test]
fn test_mid_copy_removal_simulation() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config.clone(), checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr.clone(), mount_provider);

    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    // Create a large file (256 KB)
    let large_data = vec![0xABu8; 256 * 1024];
    let clip_path = inbox_dir.join("interrupted.mp4");
    create_sample_file(&clip_path, &large_data);

    let candidate = CandidateFile {
        source_path: clip_path.clone(),
        relative_path: PathBuf::from("interrupted.mp4"),
        file_name: "interrupted.mp4".into(),
        size_bytes: large_data.len() as u64,
        extension: "mp4".into(),
    };

    // Simulate USB removal midway through copying
    let err = engine
        .copy_candidate(&candidate, || true)
        .unwrap_err();

    match err {
        StorageError::MidCopyRemoval { path, reason } => {
            assert_eq!(path, clip_path);
            assert!(reason.contains("Drive removed or communication lost mid-copy"));
        }
        _ => panic!("Expected MidCopyRemoval error, got {:?}", err),
    }

    // Verify job in JobStore is marked as Failed
    let jobs = mgr.job_store().list_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    let job = &jobs[0];
    match &job.state {
        JobState::Failed { reason } => {
            assert!(reason.contains("Drive removed"));
        }
        _ => panic!("Expected job state to be Failed, got {:?}", job.state),
    }

    // Verify no partial file was left inside the staging directory
    let dest_file = job.staging_path.join("interrupted.mp4");
    assert!(!dest_file.exists());
}

#[test]
fn test_malicious_symlink_and_os_metadata_rejection() {
    let temp_usb = TempDir::new().unwrap();
    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");
    fs::create_dir_all(&inbox_dir).unwrap();

    // 1. Legitimate video file inside inbox
    let valid_video = inbox_dir.join("valid_loop.mp4");
    create_sample_file(&valid_video, b"VALID_VIDEO_DATA");

    // 2. Video file OUTSIDE the inbox at root of USB
    let outside_video = usb_root.join("outside_ignored.mp4");
    create_sample_file(&outside_video, b"OUTSIDE_DATA");

    // 3. Sensitive target outside USB
    let sensitive_target = temp_usb.path().join("sensitive_secret.txt");
    create_sample_file(&sensitive_target, b"SECRET_PASSWORD_DATA");

    // Create symlink inside inbox pointing to outside sensitive file
    let symlink_path = inbox_dir.join("malicious_symlink.mp4");
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&sensitive_target, &symlink_path);
    }
    #[cfg(windows)]
    {
        let _ = std::os::windows::fs::symlink_file(&sensitive_target, &symlink_path);
    }

    // 4. Hidden files and dotfiles
    create_sample_file(&inbox_dir.join(".DS_Store"), b"MACOS_METADATA");
    create_sample_file(&inbox_dir.join("._valid_loop.mp4"), b"APPLEDOUBLE_HEADER");

    // 5. OS Metadata directory: System Volume Information
    let sys_vol_dir = inbox_dir.join("System Volume Information");
    fs::create_dir_all(&sys_vol_dir).unwrap();
    create_sample_file(&sys_vol_dir.join("IndexerVolumeGuid"), b"GUID_DATA");

    // 6. Unsupported extension files
    create_sample_file(&inbox_dir.join("autorun.exe"), b"PE_BINARY");
    create_sample_file(&inbox_dir.join("exploit.sh"), b"#!/bin/sh");
    create_sample_file(&inbox_dir.join("notes.txt"), b"some text");

    // Run scanner
    let report = InboxScanner::scan_inbox(usb_root).unwrap();

    // Candidates should ONLY contain the valid video
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].source_path, valid_video);
    assert_eq!(report.candidates[0].file_name, "valid_loop.mp4");

    // Check ignored OS metadata
    assert!(report
        .ignored_os_metadata
        .iter()
        .any(|p| p.file_name().unwrap() == ".DS_Store"));
    assert!(report
        .ignored_os_metadata
        .iter()
        .any(|p| p.file_name().unwrap() == "._valid_loop.mp4"));
    assert!(report
        .ignored_os_metadata
        .iter()
        .any(|p| p.file_name().unwrap() == "System Volume Information"));

    // Check unsupported files
    assert!(report
        .unsupported_files
        .iter()
        .any(|p| p.file_name().unwrap() == "autorun.exe"));
    assert!(report
        .unsupported_files
        .iter()
        .any(|p| p.file_name().unwrap() == "exploit.sh"));
    assert!(report
        .unsupported_files
        .iter()
        .any(|p| p.file_name().unwrap() == "notes.txt"));

    // Check symlink rejection (if symlink was created)
    if symlink_path.exists()
        || fs::symlink_metadata(&symlink_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        assert!(report.ignored_symlinks.contains(&symlink_path));
    }
}

#[test]
fn test_filename_collision_isolation() {
    let temp_appliance = TempDir::new().unwrap();
    let temp_usb = TempDir::new().unwrap();

    let config = StorageConfig::for_testing(temp_appliance.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(
        50 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config, checker).unwrap();
    let mount_provider = Arc::new(MockMountProvider::new());
    let engine = UsbCopyEngine::new(mgr, mount_provider);

    let usb_root = temp_usb.path();
    let inbox_dir = usb_root.join("RPI-VJ-INBOX");

    // Create two files with exact same filename in different subfolders with different contents
    let file1_path = inbox_dir.join("folder_a").join("clip.mp4");
    let file2_path = inbox_dir.join("folder_b").join("clip.mp4");

    let content1 = b"CONTENT_FOR_CLIP_A";
    let content2 = b"CONTENT_FOR_CLIP_B";

    create_sample_file(&file1_path, content1);
    create_sample_file(&file2_path, content2);

    let outcomes = engine.process_mount(usb_root).unwrap();
    assert_eq!(outcomes.len(), 2);

    let mut staged_files = Vec::new();
    for outcome in outcomes {
        match outcome {
            FileIngestOutcome::Copied(job) => {
                let staged = PathBuf::from(job.details.get("staged_file").unwrap());
                assert!(staged.exists());
                staged_files.push(staged);
            }
            _ => panic!("Expected Copied outcome"),
        }
    }

    assert_eq!(staged_files.len(), 2);
    // Ensure both staged files are in distinct job staging directories
    assert_ne!(staged_files[0], staged_files[1]);
    assert_ne!(staged_files[0].parent(), staged_files[1].parent());

    let c1 = fs::read(&staged_files[0]).unwrap();
    let c2 = fs::read(&staged_files[1]).unwrap();
    assert_ne!(c1, c2);
}
