#![cfg(windows)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn container(program: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = b"DXBC".to_vec();
    bytes.extend([0; 16]); // digest unused by the parser
    bytes.extend(1u32.to_le_bytes());
    let size = program.map_or(32, |part| 44 + part.len() as u32);
    bytes.extend(size.to_le_bytes());
    bytes.extend(u32::from(program.is_some()).to_le_bytes());
    if let Some(part) = program {
        bytes.extend(36u32.to_le_bytes());
        bytes.extend(b"ACCC");
        bytes.extend((part.len() as u32).to_le_bytes());
        bytes.extend(part);
    }
    bytes
}

/// A native compute shader with one wave32 group and only `s_endpgm`.
fn empty_compute() -> Vec<u8> {
    let packet = [0u32, 0, 0, 0x0001_0020, 0x0010_0001, 0, 0];
    let code = [0xBF81_0000u32];
    let metadata = [0x403u32, 4, 1, 0, 0, 0x40];
    let mut words = vec![u32::from_le_bytes(*b"FSPX"), 1, 3, 120];
    words.extend([0, 64, 28, 0, 6, 92, 4, 0, 7, 96, 24, 0]);
    words.extend(packet);
    words.extend(code);
    words.extend(metadata);
    let program: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    container(Some(&program))
}

fn run(root: &Path, cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_victory"))
        .args(args)
        .args(["--jobs", "1"])
        .arg(root.join("input"))
        .current_dir(root)
        .env_remove("VICTORY_CONFIG")
        .env_remove("VICTORY_WARM_SECONDS")
        .env("VICTORY_CACHE_DIR", cache)
        .env("VICTORY_DATA_DIR", root.join("data"))
        .output()
        .unwrap()
}

#[test]
fn offline_commands_do_not_report_an_unwritten_shader_as_cached() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let cache = root.join("cache");
    fs::create_dir(root.join("input")).unwrap();
    let shader = empty_compute();
    fs::write(root.join("input/shader.bin"), &shader).unwrap();
    fs::write(root.join("input/unsupported.bin"), container(None)).unwrap();

    let saved = run(root, &cache, &["shaders"]);
    assert!(saved.status.success(), "{saved:?}");
    assert!(String::from_utf8_lossy(&saved.stdout).contains("1 shaders in the cache, 1 do not translate"), "{saved:?}");

    // Recover the real key, without duplicating host/translator key construction.
    let shard = fs::read_dir(cache.join("shaders")).unwrap().next().unwrap().unwrap().path();
    let entry = fs::read_dir(shard).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(&entry).unwrap();
    fs::create_dir(&entry).unwrap();

    let failed = run(root, &cache, &["shaders"]);
    assert!(!failed.status.success(), "{failed:?}");
    assert!(String::from_utf8_lossy(&failed.stdout).contains("0 shaders in the cache, 1 do not translate"), "{failed:?}");
    assert!(String::from_utf8_lossy(&failed.stderr).contains("could not cache 1 shaders"), "{failed:?}");

    let warm = run(root, &cache, &["translate", "--warm", "all"]);
    assert!(!warm.status.success(), "{warm:?}");
    assert!(String::from_utf8_lossy(&warm.stdout).contains("0 translated now, 0 in the cache already, 1 do not translate"), "{warm:?}");
    assert!(String::from_utf8_lossy(&warm.stderr).contains("could not cache 1 shaders"), "{warm:?}");
    assert!(entry.is_dir());
}
