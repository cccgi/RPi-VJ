use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use vj_storage::disk::MockDiskSpaceChecker;
use vj_storage::{JobState, ShowStorageManager, StorageConfig, StorageError};

#[test]
fn test_disk_reserve_15_percent_enforcement() {
    let temp = TempDir::new().unwrap();
    let config = StorageConfig::for_testing(temp.path());

    // Total: 100 GB, Free: 20 GB (20% free) -> 15% reserve is 15 GB -> 5 GB allowed max
    let checker = Arc::new(MockDiskSpaceChecker::new(
        20 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
    ));
    let mgr = ShowStorageManager::with_checker(config.clone(), checker).unwrap();

    // 4 GB job -> should succeed (20 - 4 = 16 GB >= 15 GB reserve)
    assert!(mgr.check_free_space(4 * 1024 * 1024 * 1024).is_ok());

    // 6 GB job -> should fail because remaining would be 14 GB < 15 GB reserve
    let err = mgr.check_free_space(6 * 1024 * 1024 * 1024).unwrap_err();
    match err {
        StorageError::InsufficientSpace {
            required_bytes,
            free_bytes,
            total_bytes,
            reserve_percent,
        } => {
            assert_eq!(required_bytes, 6 * 1024 * 1024 * 1024);
            assert_eq!(free_bytes, 20 * 1024 * 1024 * 1024);
            assert_eq!(total_bytes, 100 * 1024 * 1024 * 1024);
            assert_eq!(reserve_percent, 15.0);
        }
        _ => panic!("Expected InsufficientSpace error, got {:?}", err),
    }

    // Attempting start_job when space insufficient fails immediately
    let low_space_checker = Arc::new(MockDiskSpaceChecker::new(
        10 * 1024 * 1024 * 1024, // 10 GB free < 15 GB reserve
        100 * 1024 * 1024 * 1024,
    ));
    let mgr_low = ShowStorageManager::with_checker(config, low_space_checker).unwrap();
    assert!(mgr_low.start_job(PathBuf::from("/media/usb/show"), 1024).is_err());
}

#[test]
fn test_immutable_generations_activation_and_rollback() {
    let temp = TempDir::new().unwrap();
    let config = StorageConfig::for_testing(temp.path());
    let mgr = ShowStorageManager::new(config.clone()).unwrap();

    // Create gen 1
    let gen1_name = "gen-20260915000000-aaaa1111";
    let gen1_path = config.shows_dir.join(gen1_name);
    std::fs::create_dir_all(&gen1_path).unwrap();
    std::fs::write(gen1_path.join("show.toml"), "version = 1\nname = 'Show 1'").unwrap();

    // Activate gen 1
    mgr.activate_generation(gen1_name).unwrap();
    let active = mgr.get_active_generation_path().unwrap().unwrap();
    assert!(active.to_string_lossy().contains(gen1_name));

    // Create gen 2
    let gen2_name = "gen-20260915010000-bbbb2222";
    let gen2_path = config.shows_dir.join(gen2_name);
    std::fs::create_dir_all(&gen2_path).unwrap();
    std::fs::write(gen2_path.join("show.toml"), "version = 2\nname = 'Show 2'").unwrap();

    // Activate gen 2
    mgr.activate_generation(gen2_name).unwrap();
    let active2 = mgr.get_active_generation_path().unwrap().unwrap();
    assert!(active2.to_string_lossy().contains(gen2_name));

    let gens = mgr.list_generations().unwrap();
    assert_eq!(gens.len(), 2);
    assert_eq!(gens[0].name, gen2_name);
    assert!(gens[0].is_active);
    assert_eq!(gens[1].name, gen1_name);
    assert!(!gens[1].is_active);

    // Rollback to gen 1
    let rolled_back_name = mgr.rollback().unwrap();
    assert_eq!(rolled_back_name, gen1_name);

    let active_after_rollback = mgr.get_active_generation_path().unwrap().unwrap();
    assert!(active_after_rollback.to_string_lossy().contains(gen1_name));

    // Prune generations keeping 2: both gen 1 and gen 2 should remain
    let pruned = mgr.prune_old_generations(2).unwrap();
    assert_eq!(pruned.len(), 0);

    // Add gen 3 and gen 4
    let gen3_name = "gen-20260915020000-cccc3333";
    let gen4_name = "gen-20260915030000-dddd4444";
    std::fs::create_dir_all(config.shows_dir.join(gen3_name)).unwrap();
    std::fs::create_dir_all(config.shows_dir.join(gen4_name)).unwrap();

    // Prune keeping 2
    let pruned_more = mgr.prune_old_generations(2).unwrap();
    assert!(pruned_more.contains(&gen2_name.to_string()) || pruned_more.contains(&gen1_name.to_string()) || pruned_more.contains(&gen3_name.to_string()));
}

#[test]
fn test_durable_job_state_transitions() {
    let temp = TempDir::new().unwrap();
    let config = StorageConfig::for_testing(temp.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(50 * 1024 * 1024 * 1024, 100 * 1024 * 1024 * 1024));
    let mgr = ShowStorageManager::with_checker(config.clone(), checker).unwrap();

    // Start job
    let job = mgr.start_job(PathBuf::from("/media/usb/show_source"), 1024 * 1024).unwrap();
    assert_eq!(job.state, JobState::Discovered);
    assert!(job.staging_path.exists());

    let job_store = mgr.job_store();

    // Transition discovered -> copying
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Copying;
        j.progress_percent = 25.0;
    }).unwrap();
    assert_eq!(job.state, JobState::Copying);

    // Transition copying -> probing
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Probing;
    }).unwrap();
    assert_eq!(job.state, JobState::Probing);

    // Transition probing -> transcoding
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Transcoding;
        j.progress_percent = 50.0;
    }).unwrap();
    assert_eq!(job.state, JobState::Transcoding);

    // Transition transcoding -> validating
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Validating;
        j.progress_percent = 90.0;
    }).unwrap();
    assert_eq!(job.state, JobState::Validating);

    // Transition validating -> ready
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Ready;
        j.progress_percent = 100.0;
    }).unwrap();
    assert_eq!(job.state, JobState::Ready);

    // Promote staging to new generation
    let gen_name = ShowStorageManager::generate_generation_name(b"test_show");
    let gen_path = mgr.promote_staging_to_generation(job.id, &gen_name).unwrap();
    assert!(gen_path.exists());

    // Transition ready -> activating
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Activating;
        j.target_generation = Some(gen_name.clone());
    }).unwrap();
    assert_eq!(job.state, JobState::Activating);

    mgr.activate_generation(&gen_name).unwrap();

    // Transition activating -> complete
    let job = job_store.transition_job(job.id, |j| {
        j.state = JobState::Complete;
    }).unwrap();
    assert_eq!(job.state, JobState::Complete);

    // Verify invalid transition from Complete -> Copying is rejected
    let inv_res = job_store.transition_job(job.id, |j| {
        j.state = JobState::Copying;
    });
    assert!(inv_res.is_err());
}

#[test]
fn test_boot_recovery_and_quarantine() {
    let temp = TempDir::new().unwrap();
    let config = StorageConfig::for_testing(temp.path());
    let checker = Arc::new(MockDiskSpaceChecker::new(50 * 1024 * 1024 * 1024, 100 * 1024 * 1024 * 1024));
    
    // Simulate active show existing before crash
    let old_gen_name = "gen-20260915000000-existing";
    let old_gen_path = config.shows_dir.join(old_gen_name);
    std::fs::create_dir_all(&old_gen_path).unwrap();
    std::fs::write(old_gen_path.join("show.toml"), "name = 'Rock solid show'").unwrap();

    let mgr = ShowStorageManager::with_checker(config.clone(), checker.clone()).unwrap();
    mgr.activate_generation(old_gen_name).unwrap();

    // Start an ingest job and simulate power cut during Transcoding
    let job = mgr.start_job(PathBuf::from("/media/usb/new_show"), 5000).unwrap();
    let _ = mgr.job_store().transition_job(job.id, |j| {
        j.state = JobState::Copying;
    }).unwrap();
    let _ = mgr.job_store().transition_job(job.id, |j| {
        j.state = JobState::Probing;
    }).unwrap();
    let _ = mgr.job_store().transition_job(job.id, |j| {
        j.state = JobState::Transcoding;
    }).unwrap();

    // Put some partial files in the staging dir
    std::fs::write(job.staging_path.join("clip1.mp4.partial"), b"partial video bytes").unwrap();

    // Simulate system restart: Create a fresh manager instance and run boot recovery
    let rebooted_mgr = ShowStorageManager::with_checker(config.clone(), checker).unwrap();
    let report = rebooted_mgr.run_boot_recovery().unwrap();

    assert_eq!(report.recovered_jobs.len(), 1);
    match &report.recovered_jobs[0].state {
        JobState::Failed { reason } => {
            assert!(reason.contains("Interrupted"));
        }
        _ => panic!("Expected job to be marked failed on boot recovery"),
    }

    // Staging path should no longer exist under original job id
    assert!(!job.staging_path.exists());
    assert_eq!(report.quarantined_paths.len(), 1);
    assert!(PathBuf::from(&report.quarantined_paths[0]).exists());

    // Verify existing active show was completely untouched and remains active
    let active = rebooted_mgr.get_active_generation_path().unwrap().unwrap();
    assert!(active.to_string_lossy().contains(old_gen_name));
}
