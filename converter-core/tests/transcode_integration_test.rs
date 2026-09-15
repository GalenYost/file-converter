use std::time::Duration;
use tokio::time::sleep;

use converter_core::binaries::create_quiet_cmd;
use converter_core::engine::{ConversionEngine, EngineConfig};
use converter_core::format::MediaFormat;
use converter_core::job::{ConversionJob, JobStatus};

#[tokio::test]
async fn test_real_audio_transcoding() {
    let temp_dir = std::env::temp_dir().join("file_converter_test");
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();

    let input_wav = temp_dir.join("test_tone.wav");
    let output_mp3 = temp_dir.join("test_tone.mp3");

    let _ = tokio::fs::remove_file(&input_wav).await;
    let _ = tokio::fs::remove_file(&output_mp3).await;

    // Generate a 1-second 440Hz test sine tone using ffmpeg
    let gen_status = create_quiet_cmd("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            input_wav.to_str().unwrap(),
        ])
        .output()
        .await;

    if gen_status.is_err() || !gen_status.unwrap().status.success() {
        eprintln!("Skipping integration test: ffmpeg not available to generate test tone");
        return;
    }

    let engine = ConversionEngine::new(EngineConfig::default());
    let mut rx = engine.subscribe();

    let job = ConversionJob::new(input_wav.clone(), output_mp3.clone(), MediaFormat::Mp3);
    let job_id = engine.submit_job(job).await;

    // Listen to events until completion or timeout
    let mut completed = false;
    let timeout = sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            Ok(event) = rx.recv() => {
                match event {
                    converter_core::job::JobEvent::Completed { id, .. } if id == job_id => {
                        completed = true;
                        break;
                    }
                    converter_core::job::JobEvent::Failed { id, error } if id == job_id => {
                        panic!("Job failed: {}", error);
                    }
                    _ => {}
                }
            }
        }
    }

    assert!(completed, "Job should have completed successfully");
    assert!(output_mp3.exists(), "Output MP3 file must exist");

    let final_job = engine.get_job(job_id).await.expect("Job should exist");
    assert!(matches!(final_job.status, JobStatus::Completed { .. }));

    // Cleanup
    let _ = tokio::fs::remove_file(&input_wav).await;
    let _ = tokio::fs::remove_file(&output_mp3).await;
}
