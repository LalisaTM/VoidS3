//! Real HTTP S3 integration. Explicit opt-in; never reads production profiles/keychain.
use aws_sdk_s3::primitives::ByteStream;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
use voids3_core::{Credentials, MIB, Profile, Provider, TransferSettings};
use voids3_s3::S3;
use voids3_storage::Store;
use voids3_transfer::batch::FeedState;
use voids3_transfer::upload::ConflictAction;
use voids3_transfer::{Direction, Engine, Fingerprint, Status, Transfer};

fn client(bucket: String) -> S3 {
    let endpoint = std::env::var("VOIDS3_TEST_ENDPOINT")
        .expect("Set VOIDS3_TEST_ENDPOINT for explicitly opted-in tests");
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Integration fixture".into(),
        provider: Provider::Custom,
        allow_http: endpoint.starts_with("http://"),
        endpoint,
        region: std::env::var("VOIDS3_TEST_REGION").unwrap_or("us-east-1".into()),
        bucket,
        prefix: String::new(),
        path_style: true,
        read_only: false,
    };
    S3::new(
        profile,
        Credentials {
            access_key: std::env::var("VOIDS3_TEST_ACCESS_KEY")
                .expect("Set VOIDS3_TEST_ACCESS_KEY"),
            secret_key: std::env::var("VOIDS3_TEST_SECRET_KEY")
                .expect("Set VOIDS3_TEST_SECRET_KEY"),
            session_token: std::env::var("VOIDS3_TEST_SESSION_TOKEN").ok(),
        },
        1,
    )
    .unwrap()
}
fn settings() -> TransferSettings {
    TransferSettings {
        concurrent_files: 2,
        concurrent_parts: 2,
        max_requests: 3,
        part_size: 5 * MIB,
        multipart_threshold: 5 * MIB,
        retries: 1,
    }
}
async fn wait(engine: &Engine, id: &str) -> Transfer {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let job = engine
                .list()
                .unwrap()
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            if matches!(
                job.status,
                Status::Completed
                    | Status::Failed
                    | Status::Cancelled
                    | Status::Conflict
                    | Status::Skipped
            ) {
                return job;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("transfer timed out")
}
fn job(s3: &S3, path: PathBuf, key: String, size: u64) -> Transfer {
    Transfer {
        id: uuid::Uuid::new_v4().to_string(),
        profile_id: s3.profile.id.clone(),
        target: s3.profile.clone(),
        direction: Direction::Upload,
        local_path: path,
        key,
        size,
        bytes_done: 0,
        status: Status::Uploading,
        error: None,
        created: 0,
        part_size: 5 * MIB,
        upload_id: None,
        parts: BTreeMap::new(),
        source: None,
        remote_etag: None,
        batch_id: None,
        conflict: None,
        upload_condition: Default::default(),
        priority: 0,
        settings: settings(),
    }
}

#[tokio::test]
#[ignore = "Requires explicitly configured local S3 or disposable live bucket"]
async fn real_s3_transfers_and_recovery() {
    let owned_bucket = std::env::var("VOIDS3_TEST_BUCKET").is_err();
    let s3 = client(
        std::env::var("VOIDS3_TEST_BUCKET")
            .unwrap_or_else(|_| format!("voids3-test-{}", uuid::Uuid::new_v4())),
    );
    if owned_bucket {
        s3.client
            .create_bucket()
            .bucket(&s3.profile.bucket)
            .send()
            .await
            .unwrap();
    }
    let prefix = format!("voids3-integration/{}/", uuid::Uuid::new_v4());
    let result = exercise(&s3, &prefix).await;
    // Restrict cleanup to this run's random prefix, including failed multipart sessions.
    let mut cleanup_errors = Vec::new();
    match s3.scan(&prefix).await {
        Ok(entries) => {
            for entry in entries {
                if let Err(e) = s3.delete(&entry.path, None).await {
                    cleanup_errors.push(e.to_string());
                }
            }
        }
        Err(e) => cleanup_errors.push(e.to_string()),
    }
    match s3
        .client
        .list_multipart_uploads()
        .bucket(&s3.profile.bucket)
        .prefix(&prefix)
        .send()
        .await
    {
        Ok(uploads) => {
            for upload in uploads.uploads() {
                if let Err(e) = s3
                    .client
                    .abort_multipart_upload()
                    .bucket(&s3.profile.bucket)
                    .key(upload.key().unwrap_or_default())
                    .upload_id(upload.upload_id().unwrap_or_default())
                    .send()
                    .await
                {
                    cleanup_errors.push(format!("Multipart cleanup: {e}"));
                }
            }
        }
        Err(_) => cleanup_errors.push(
            "Cannot list multipart sessions for cleanup; check test bucket permissions".into(),
        ),
    }
    if owned_bucket
        && let Err(e) = s3
            .client
            .delete_bucket()
            .bucket(&s3.profile.bucket)
            .send()
            .await
    {
        cleanup_errors.push(format!("Test bucket cleanup: {e}"));
    }
    assert!(
        cleanup_errors.is_empty(),
        "Cleanup failures: {cleanup_errors:?}; test result: {result:?}"
    );
    result.unwrap();
}

async fn exercise(s3: &S3, prefix: &str) -> Result<(), Box<dyn std::error::Error>> {
    s3.test().await?;
    let dir = tempfile::tempdir()?;
    let store = Arc::new(Store::open(&dir.path().join("queue.db"))?);
    let engine = Engine::new(store.clone(), settings())?;
    let small = dir.path().join("small.txt");
    tokio::fs::write(&small, b"Real HTTP transfer, no fabricated data.").await?;
    let large = dir.path().join("large.bin");
    let content: Vec<u8> = (0..11 * MIB + 123)
        .map(|i| ((i * 31 + i / 1024) % 251) as u8)
        .collect();
    tokio::fs::write(&large, &content).await?;
    let small_key = format!("{prefix}small.txt");
    let large_key = format!("{prefix}large.bin");
    // Resume queues workers synchronously. On this current-thread runtime, a
    // global stop must reach every worker before any of them obtains a permit.
    let stop_store = Arc::new(Store::open(&dir.path().join("global-stop.db"))?);
    let mut stopping = Vec::new();
    for index in 0..5 {
        let mut queued = job(s3, small.clone(), format!("{prefix}stopped-{index}.txt"), 0);
        queued.status = Status::Paused;
        queued.source = Some(Fingerprint::read(&small).await?);
        stop_store.put("transfers", &queued.id, &queued)?;
        stopping.push(queued);
    }
    let stopped_engine = Engine::new(stop_store.clone(), settings())?;
    for queued in &stopping {
        stopped_engine.resume(s3.clone(), &queued.id)?;
    }
    assert_eq!(stopped_engine.interrupt_all(false)?.len(), stopping.len());
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if stopped_engine
                .list()
                .unwrap()
                .iter()
                .all(|j| j.status == Status::Paused)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    for queued in &stopping {
        stopped_engine.resume(s3.clone(), &queued.id)?;
    }
    assert_eq!(stopped_engine.interrupt_all(true)?.len(), stopping.len());
    for queued in &stopping {
        assert_eq!(
            wait(&stopped_engine, &queued.id).await.status,
            Status::Cancelled
        );
        assert!(s3.head(&queued.key).await.is_err());
    }
    assert!(
        stop_store
            .list::<Transfer>("transfers")?
            .iter()
            .all(|j| j.status == Status::Cancelled)
    );
    let one = engine
        .enqueue(
            s3.clone(),
            Direction::Upload,
            small.clone(),
            small_key.clone(),
            settings(),
        )
        .await?;
    let two = engine
        .enqueue(
            s3.clone(),
            Direction::Upload,
            large.clone(),
            large_key.clone(),
            settings(),
        )
        .await?;
    assert_eq!(wait(&engine, &one).await.status, Status::Completed);
    let large_job = wait(&engine, &two).await;
    assert_eq!(large_job.status, Status::Completed, "{:?}", large_job.error);
    assert_eq!(large_job.parts.len(), 3);
    assert_eq!(large_job.bytes_done, content.len() as u64);
    assert_eq!(s3.list(prefix, None, false).await?.entries.len(), 2);
    assert_eq!(
        s3.preview_text(&small_key).await?,
        "Real HTTP transfer, no fabricated data."
    );
    let original = s3.head(&small_key).await?;
    let duplicate = engine
        .enqueue(
            s3.clone(),
            Direction::Upload,
            small.clone(),
            small_key.clone(),
            settings(),
        )
        .await?;
    assert_eq!(
        wait(&engine, &duplicate).await.status,
        Status::Failed,
        "Conditional upload must not overwrite"
    );
    assert_eq!(s3.head(&small_key).await?.etag, original.etag);

    let downloaded = dir.path().join("download.bin");
    let download = engine
        .enqueue(
            s3.clone(),
            Direction::Download,
            downloaded.clone(),
            large_key.clone(),
            settings(),
        )
        .await?;
    assert_eq!(wait(&engine, &download).await.status, Status::Completed);
    assert_eq!(tokio::fs::read(&downloaded).await?, content);
    assert!(
        engine
            .enqueue(
                s3.clone(),
                Direction::Download,
                downloaded,
                large_key.clone(),
                settings()
            )
            .await
            .is_err()
    );

    // Simulate a process interruption after a provider-accepted first part but before SQLite saved it.
    let recovery_key = format!("{prefix}recovered.bin");
    let upload = s3
        .client
        .create_multipart_upload()
        .bucket(&s3.profile.bucket)
        .key(&recovery_key)
        .send()
        .await?;
    let upload_id = upload.upload_id().ok_or("missing upload ID")?.to_owned();
    s3.client
        .upload_part()
        .bucket(&s3.profile.bucket)
        .key(&recovery_key)
        .upload_id(&upload_id)
        .part_number(1)
        .body(ByteStream::from(content[..5 * MIB as usize].to_vec()))
        .send()
        .await?;
    let mut recovered = job(
        s3,
        large.clone(),
        recovery_key.clone(),
        content.len() as u64,
    );
    recovered.upload_id = Some(upload_id);
    recovered.source = Some(Fingerprint::read(&large).await?);
    store.put("transfers", &recovered.id, &recovered)?;
    let restarted = Engine::new(store.clone(), settings())?;
    assert_eq!(
        restarted
            .list()?
            .iter()
            .find(|j| j.id == recovered.id)
            .unwrap()
            .status,
        Status::Paused
    );
    restarted.resume(s3.clone(), &recovered.id)?;
    assert_eq!(
        wait(&restarted, &recovered.id).await.status,
        Status::Completed
    );
    let recovered_body = s3
        .client
        .get_object()
        .bucket(&s3.profile.bucket)
        .key(&recovery_key)
        .send()
        .await?
        .body
        .collect()
        .await?
        .into_bytes();
    assert_eq!(recovered_body.as_ref(), content.as_slice());

    // Download recovery uses actual partial file length and a conditional range request.
    let resumed_path = dir.path().join("resumed.bin");
    let remote = s3.head(&large_key).await?;
    let mut partial = job(s3, resumed_path.clone(), large_key.clone(), remote.size);
    partial.direction = Direction::Download;
    partial.status = Status::Downloading;
    partial.remote_etag = Some(remote.etag);
    tokio::fs::write(
        resumed_path.with_file_name(format!(".voids3-{}.part", partial.id)),
        &content[..128 * 1024],
    )
    .await?;
    store.put("transfers", &partial.id, &partial)?;
    let download_restart = Engine::new(store.clone(), settings())?;
    download_restart.resume(s3.clone(), &partial.id)?;
    assert_eq!(
        wait(&download_restart, &partial.id).await.status,
        Status::Completed
    );
    assert_eq!(tokio::fs::read(&resumed_path).await?, content);

    // A modified source is rejected before any resumed part is sent; cancel aborts the session.
    let changed_path = dir.path().join("changed.bin");
    tokio::fs::write(&changed_path, &content).await?;
    let changed_key = format!("{prefix}changed.bin");
    let upload = s3
        .client
        .create_multipart_upload()
        .bucket(&s3.profile.bucket)
        .key(&changed_key)
        .send()
        .await?;
    let mut changed = job(
        s3,
        changed_path.clone(),
        changed_key.clone(),
        content.len() as u64,
    );
    changed.upload_id = upload.upload_id().map(str::to_owned);
    changed.source = Some(Fingerprint::read(&changed_path).await?);
    store.put("transfers", &changed.id, &changed)?;
    tokio::fs::write(&changed_path, b"source was modified").await?;
    let changed_engine = Engine::new(store.clone(), settings())?;
    changed_engine.resume(s3.clone(), &changed.id)?;
    let failed = wait(&changed_engine, &changed.id).await;
    assert_eq!(failed.status, Status::Failed);
    assert!(
        failed
            .error
            .unwrap_or_default()
            .contains("Source file changed")
    );
    changed_engine.cancel(s3.clone(), &changed.id)?;
    assert_eq!(
        wait(&changed_engine, &changed.id).await.status,
        Status::Cancelled
    );
    assert!(
        s3.client
            .list_parts()
            .bucket(&s3.profile.bucket)
            .key(&changed_key)
            .set_upload_id(changed.upload_id)
            .send()
            .await
            .is_err()
    );

    let copied = format!("{prefix}copied.txt");
    s3.copy(&small_key, &copied, &original.etag).await?;
    assert_eq!(
        s3.preview_text(&copied).await?,
        "Real HTTP transfer, no fabricated data."
    );
    s3.delete(&copied, Some(&s3.head(&copied).await?.etag))
        .await?;
    assert!(s3.head(&copied).await.is_err());
    let url = s3.presign(&small_key, false, 60).await?;
    assert!(url.contains("X-Amz-Signature="));
    assert_eq!(
        reqwest::get(url).await?.error_for_status()?.text().await?,
        "Real HTTP transfer, no fabricated data."
    );
    s3.folder(&format!("{prefix}empty/")).await?;
    exercise_upload_conflicts(s3, &format!("{prefix}conflicts/")).await?;
    Ok(())
}

async fn idle(engine: &Engine) {
    tokio::time::timeout(Duration::from_secs(90), async {
        loop {
            let summary = engine.summary().unwrap();
            if summary.active == 0
                && summary
                    .batches
                    .iter()
                    .all(|batch| batch.state != FeedState::Running)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("batch did not become idle");
}
async fn exercise_upload_conflicts(
    s3: &S3,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let store = Arc::new(Store::open(&dir.path().join("queue.db"))?);
    let engine = Engine::new(store.clone(), settings())?;
    let folder = dir.path().join("photos");
    tokio::fs::create_dir(&folder).await?;
    for index in 0..200 {
        tokio::fs::write(
            folder.join(format!("photo-{index:04}.jpg")),
            b"incoming photo",
        )
        .await?;
    }
    let content = vec![137_u8; (6 * MIB + 31) as usize];
    tokio::fs::write(folder.join("large.bin"), &content).await?;
    let photo_key = format!("{prefix}photos/photo-0000.jpg");
    let large_key = format!("{prefix}photos/large.bin");
    for key in [
        &photo_key,
        &large_key,
        &format!("{prefix}photos/photo-0000 (1).jpg"),
    ] {
        s3.client
            .put_object()
            .bucket(&s3.profile.bucket)
            .key(key)
            .body(ByteStream::from_static(b"old destination"))
            .send()
            .await?;
    }
    let batch = engine.start_batch(s3.clone(), vec![folder.clone()], prefix.into(), settings())?;
    // No batch-wide remote scan: a worker completes while discovery still has
    // undiscovered files, and waiting conflicts do not stall fresh uploads.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let summary = engine.summary().unwrap();
            let feed = summary
                .batches
                .iter()
                .find(|feed| feed.id == batch)
                .unwrap();
            if summary.counts.get("completed").copied().unwrap_or(0) > 0 {
                assert!(
                    feed.discovered < 201,
                    "first completion waited for all discovery"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await?;
    idle(&engine).await;
    let summary = engine.summary()?;
    assert_eq!(summary.total, 201);
    assert_eq!(summary.counts.get("conflict"), Some(&2));
    assert_eq!(summary.counts.get("completed"), Some(&199));
    let conflicts = engine.page("conflict", 0, 100)?.items;
    let photo = conflicts.iter().find(|job| job.key == photo_key).unwrap();
    let large = conflicts.iter().find(|job| job.key == large_key).unwrap();
    engine.resolve_conflicts(s3.clone(), &photo.id, ConflictAction::KeepBoth, false)?;
    engine.resolve_conflicts(s3.clone(), &large.id, ConflictAction::Replace, false)?;
    // Resolvers are asynchronous, so wait for their outcomes rather than only
    // looking at the current active count between registration and scheduling.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if engine.transfer(&photo.id).unwrap().status == Status::Completed
                && engine.transfer(&large.id).unwrap().status == Status::Completed
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(s3.preview_text(&photo_key).await?, "old destination");
    assert_eq!(
        s3.preview_text(&format!("{prefix}photos/photo-0000 (2).jpg"))
            .await?,
        "incoming photo"
    );
    let actual = s3
        .client
        .get_object()
        .bucket(&s3.profile.bucket)
        .key(&large_key)
        .send()
        .await?
        .body
        .collect()
        .await?
        .into_bytes();
    assert_eq!(actual.as_ref(), content.as_slice());

    // Apply-to-all is a backend batch policy and also handles files that have
    // not yet reached a worker. Skipping never changes remote contents.
    let next = engine.start_batch(s3.clone(), vec![folder.clone()], prefix.into(), settings())?;
    let first = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(job) = engine
                .page("conflict", 0, 100)
                .unwrap()
                .items
                .into_iter()
                .find(|job| job.batch_id.as_deref() == Some(&next))
            {
                break job;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await?;
    engine.resolve_conflicts(s3.clone(), &first.id, ConflictAction::Skip, true)?;
    tokio::time::timeout(Duration::from_secs(45), async {
        loop {
            let summary = engine.summary().unwrap();
            if summary.counts.get("skipped") == Some(&201) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(s3.preview_text(&photo_key).await?, "old destination");

    // Small-file replacement, changed-source refusal, and a stale ETag on a
    // queued resumed replacement all retain conditional-write protection.
    let single = folder.join("photo-0000.jpg");
    engine.start_batch(
        s3.clone(),
        vec![single.clone()],
        format!("{prefix}photos/"),
        settings(),
    )?;
    idle(&engine).await;
    let waiting = engine
        .page("conflict", 0, 100)?
        .items
        .into_iter()
        .find(|job| job.key == photo_key)
        .unwrap();
    tokio::fs::write(&single, b"changed source after conflict").await?;
    engine.resolve_conflicts(s3.clone(), &waiting.id, ConflictAction::Replace, false)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if engine
                .transfer(&waiting.id)
                .unwrap()
                .error
                .as_ref()
                .is_some_and(|error| error.contains("Source file changed"))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    engine.resolve_conflicts(s3.clone(), &waiting.id, ConflictAction::Skip, false)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if engine.transfer(&waiting.id).unwrap().status == Status::Skipped {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(s3.preview_text(&photo_key).await?, "old destination");
    engine.start_batch(
        s3.clone(),
        vec![single.clone()],
        format!("{prefix}photos/"),
        settings(),
    )?;
    idle(&engine).await;
    let waiting = engine.page("conflict", 0, 100)?.items.remove(0);
    engine.resolve_conflicts(s3.clone(), &waiting.id, ConflictAction::Replace, false)?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if engine.transfer(&waiting.id).unwrap().status == Status::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(
        s3.preview_text(&photo_key).await?,
        "changed source after conflict"
    );

    // Stop discovery before its first poll. It must not add files after a
    // global stop, and an updater guard must reject resuming paused discovery.
    let stopped = engine.start_batch(
        s3.clone(),
        vec![folder],
        format!("{prefix}stopped/"),
        settings(),
    )?;
    engine.interrupt_all(false)?;
    let guard = engine.begin_maintenance()?;
    assert!(engine.resume_batches().is_err());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        engine
            .summary()?
            .batches
            .iter()
            .find(|feed| feed.id == stopped)
            .unwrap()
            .discovered,
        0
    );
    drop(guard);
    engine.resume_batches()?;
    engine.interrupt_all(true)?;
    engine.cancel_idle()?;
    idle(&engine).await;
    let count = engine.summary()?.total;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        engine.summary()?.total,
        count,
        "discovery continued after cancel all"
    );
    assert!(s3.scan(&format!("{prefix}stopped/")).await?.is_empty());
    let invalid = client(format!("voids3-missing-{}", uuid::Uuid::new_v4()));
    let failed_id = engine.start_batch(
        invalid,
        vec![dir.path().join("photos")],
        format!("{prefix}invalid/"),
        settings(),
    )?;
    idle(&engine).await;
    let summary = engine.summary()?;
    let failed = summary
        .batches
        .iter()
        .find(|batch| batch.id == failed_id)
        .unwrap();
    assert_eq!(failed.state, FeedState::Failed);
    assert!(
        failed.discovered < 201,
        "connection failure did not stop further discovery"
    );
    Ok(())
}
