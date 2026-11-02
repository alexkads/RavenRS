//! `ModelFolder` against a small local server, and which models fit.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use ravenrs_ai::{LocalModel, Machine, ModelError, ModelFamily, ModelFile, ModelFolder, ModelState};
use sha2::{Digest, Sha256};

const GB: u64 = 1_000_000_000;

/// Serves `body` to each request, on a port of its own.
fn serve(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port");
    let port = listener.local_addr().expect("address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let head =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}/model")
}

fn sha(body: &[u8]) -> String {
    Sha256::digest(body).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The test's values outlive it: a model's fields are `'static` for its catalog.
fn model(url: String, bytes: u64, hash: String) -> LocalModel {
    let file = ModelFile {
        name: "model.gguf",
        url: Box::leak(url.into_boxed_str()),
        bytes,
        sha256: Box::leak(hash.into_boxed_str()),
    };
    LocalModel {
        id: "test",
        name: "Test",
        license: "MIT",
        family: ModelFamily::Qwen3,
        memory: bytes + GB,
        understood: None,
        files: Box::leak(Box::new([file])),
    }
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

#[test]
fn it_downloads_checks_and_removes() {
    let body = vec![7_u8; 300_000];
    let test = model(serve(body.clone()), body.len() as u64, sha(&body));
    let dir = tempfile::tempdir().expect("folder");
    let folder = ModelFolder::new(dir.path().join("models"));
    assert_eq!(folder.state(&test), ModelState::Absent);

    let seen = Arc::new(AtomicU64::new(0));
    let progress = seen.clone();
    folder
        .download(&test, &move |received, _| progress.store(received, Ordering::Relaxed), &never())
        .expect("download");
    assert_eq!(seen.load(Ordering::Relaxed), 300_000);
    assert_eq!(folder.state(&test), ModelState::Installed);
    let path = folder.path_of(&test, &test.files[0]);
    assert_eq!(std::fs::read(path).expect("file"), body);

    folder.remove(&test).expect("remove");
    assert_eq!(folder.state(&test), ModelState::Absent);
    folder.remove(&test).expect("removing nothing is fine");
}

#[test]
fn a_file_with_the_wrong_hash_is_not_installed() {
    let body = vec![7_u8; 1000];
    let test = model(serve(body), 1000, "0".repeat(64));
    let dir = tempfile::tempdir().expect("folder");
    let folder = ModelFolder::new(dir.path());
    assert_eq!(folder.download(&test, &|_, _| {}, &never()), Err(ModelError::WrongFile));
    assert_eq!(folder.state(&test), ModelState::Absent);
    let left = std::fs::read_dir(dir.path().join("test")).expect("its folder").count();
    assert_eq!(left, 0, "nothing stays behind");
}

#[test]
fn cancelling_leaves_nothing_behind() {
    let body = vec![7_u8; 300_000];
    let test = model(serve(body.clone()), body.len() as u64, sha(&body));
    let dir = tempfile::tempdir().expect("folder");
    let folder = ModelFolder::new(dir.path());
    let result = folder.download(&test, &|_, _| {}, &AtomicBool::new(true));
    assert_eq!(result, Err(ModelError::Cancelled));
    assert_eq!(folder.state(&test), ModelState::Absent);
    assert_eq!(std::fs::read_dir(dir.path().join("test")).expect("its folder").count(), 0);
}

#[test]
fn no_connection_says_so() {
    let test = model("http://127.0.0.1:9/model".into(), 10, "0".repeat(64));
    let dir = tempfile::tempdir().expect("folder");
    let result = ModelFolder::new(dir.path()).download(&test, &|_, _| {}, &never());
    assert!(matches!(result, Err(ModelError::Network(_))), "{result:?}");
}

fn sized(bytes: u64) -> LocalModel {
    model("http://127.0.0.1:9/".into(), bytes, "0".repeat(64))
}

#[test]
fn only_what_fits_is_compatible() {
    let big = Machine { memory: 16 * GB, free_disk: 100 * GB };
    assert!(sized(GB).fits(&big, false));

    let little_memory = Machine { memory: 2 * GB, free_disk: 100 * GB };
    assert!(!sized(GB).fits(&little_memory, false), "not enough memory");
    assert!(!sized(GB).fits(&little_memory, true), "being there does not make it run");

    let full_disk = Machine { memory: 16 * GB, free_disk: GB };
    assert!(!sized(GB).fits(&full_disk, false), "no room to download it");
    assert!(sized(GB).fits(&full_disk, true), "but it can stay if it is there");
}

#[test]
fn this_machine_has_memory_and_a_disk() {
    let dir = tempfile::tempdir().expect("folder");
    let machine = Machine::detect(dir.path());
    assert!(machine.memory > GB);
    assert!(machine.free_disk > 0);
}
