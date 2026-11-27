#![cfg(all(windows, feature = "diagnostics"))]

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn compute_shader() -> Vec<u8> {
    // A native wave32 compute group whose only instruction is s_endpgm.
    let words = [
        u32::from_le_bytes(*b"FSPX"), 1, 3, 120,
        0, 64, 28, 0, 6, 92, 4, 0, 7, 96, 24, 0,
        0, 0, 0, 0x0001_0020, 0x0010_0001, 0, 0,
        0xBF81_0000, 0x403, 4, 1, 0, 0, 0x40,
    ];
    let program: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let mut bytes = b"DXBC".to_vec();
    bytes.extend([0; 16]); // digest unused by the parser
    for word in [1, 44 + program.len() as u32, 1, 36] {
        bytes.extend(word.to_le_bytes());
    }
    bytes.extend(b"ACCC");
    bytes.extend((program.len() as u32).to_le_bytes());
    bytes.extend(program);
    bytes
}

fn write_recipe(dir: &Path, key: u8, shader: &[u8]) {
    let mut recipe = vec![key; 16];
    recipe.extend([1, 5]); // one stage, compute
    recipe.extend(blake3::hash(shader).as_bytes());
    recipe.push(0); // no graphics state
    let mut file = fs::OpenOptions::new().create(true).append(true)
        .open(dir.join("known-pipelines.bin")).unwrap();
    file.write_all(&(recipe.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&recipe).unwrap();
}

fn shader_record(shader: &[u8]) -> Vec<u8> {
    let mut record = vec![5];
    record.extend(blake3::hash(shader).as_bytes());
    record.extend((shader.len() as u32).to_le_bytes());
    record.extend(shader);
    record
}

fn warm(root: &Path) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_victory"));
    // Isolate the fixture from the operator's adapter and translation settings.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().to_ascii_uppercase().starts_with("VICTORY_") {
            command.env_remove(key);
        }
    }
    let output = command
        .args(["warm", "--jobs", "1"])
        .current_dir(root)
        .env("VICTORY_CACHE_DIR", root)
        .env("VICTORY_DATA_DIR", root.join("data"))
        // The debug-only switch also works on hosts without a discrete GPU.
        .env("VICTORY_D3D12_WARP", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn a_pipeline_with_a_missing_shader_is_retried_after_the_book_is_repaired() {
    let cache = tempfile::tempdir().unwrap();
    let root = cache.path();
    let dir = root.join("pipelines");
    fs::create_dir(&dir).unwrap();
    let shader = compute_shader();
    let mut other = shader.clone();
    other[4] = 1; // distinguish the recorded containers without changing the program
    write_recipe(&dir, 0x11, &shader);
    write_recipe(&dir, 0x22, &other);
    let book = dir.join("known-shaders.bin");
    fs::write(&book, shader_record(&shader)).unwrap();

    let first = warm(root);
    let done = dir.join("warm-done.txt");
    let recorded = fs::read_to_string(&done).unwrap();
    assert!(first.contains("1 made in") && first.contains("1 have no host pipeline"), "{first}");
    assert_eq!(recorded.lines().count(), 1, "{recorded}");
    assert!(recorded.trim_end().ends_with(&"11".repeat(16)), "{recorded}");
    assert!(fs::read(dir.join("warm-bad.txt")).unwrap().is_empty());

    let fingerprint = recorded.split_whitespace().next().unwrap().strip_prefix("made-v1:").unwrap();
    let mut history = fs::OpenOptions::new().append(true).open(&done).unwrap();
    writeln!(history, "{fingerprint} {}", "22".repeat(16)).unwrap();
    drop(history);
    fs::OpenOptions::new().append(true).open(&book).unwrap().write_all(&shader_record(&other)).unwrap();
    let repaired = warm(root);
    assert!(repaired.contains("1 to make") && repaired.contains("1 made in"), "{repaired}");
    let recorded = fs::read_to_string(&done).unwrap();
    assert_eq!(recorded.lines().filter(|line| line.starts_with("made-v1:")).count(), 2, "{recorded}");
    assert!(recorded.trim_end().ends_with(&"22".repeat(16)), "{recorded}");
    assert!(warm(root).contains("0 to make"));
}