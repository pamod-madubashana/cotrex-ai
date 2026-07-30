use std::io::{Read, Write};
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::model_manager::error::ModelManagerError;
use crate::model_manager::registry::ModelDefinition;
use crate::model_manager::storage;

pub type ProgressCallback = fn(u64, u64);

pub fn download_model(
    model: &ModelDefinition,
    on_progress: Option<ProgressCallback>,
) -> Result<PathBuf, ModelManagerError> {
    let final_path = storage::model_file_path(&model.filename)?;

    if final_path.exists() {
        verify_gguf_magic(&final_path)?;
        if let Some(ref expected) = model.sha256 {
            verify_sha256(&final_path, expected)?;
        }
        return Ok(final_path);
    }

    let part_path = storage::model_file_path(&format!("{}.part", model.filename))?;

    download_url(&model.url, &part_path, model.size, on_progress)?;

    verify_gguf_magic(&part_path)?;

    if let Some(ref expected) = model.sha256 {
        verify_sha256(&part_path, expected)?;
    }

    verify_file_size(&part_path, model.size)?;

    std::fs::rename(&part_path, &final_path)?;

    Ok(final_path)
}

fn download_url(
    url: &str,
    dest: &PathBuf,
    total_size: u64,
    on_progress: Option<ProgressCallback>,
) -> Result<(), ModelManagerError> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| ModelManagerError::DownloadFailed(format!("request failed: {e}")))?;

    let status = response.status();
    if status != 200 {
        return Err(ModelManagerError::DownloadFailed(format!("HTTP {status}")));
    }

    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(dest)?;
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        if let Some(cb) = on_progress {
            cb(downloaded, total_size);
        }
    }

    file.flush()?;
    Ok(())
}

fn verify_gguf_magic(path: &PathBuf) -> Result<(), ModelManagerError> {
    let mut file = std::fs::File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        return Err(ModelManagerError::ChecksumMismatch {
            expected: "GGUF magic bytes".into(),
            actual: format!("{magic:?}"),
        });
    }
    Ok(())
}

fn verify_sha256(path: &PathBuf, expected: &str) -> Result<(), ModelManagerError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        return Err(ModelManagerError::ChecksumMismatch {
            expected: expected.into(),
            actual,
        });
    }
    Ok(())
}

fn verify_file_size(path: &PathBuf, expected: u64) -> Result<(), ModelManagerError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() != expected {
        return Err(ModelManagerError::ChecksumMismatch {
            expected: format!("{expected} bytes"),
            actual: format!("{} bytes", metadata.len()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_gguf_magic_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.gguf");
        std::fs::write(&path, b"GGUF some model data").unwrap();
        assert!(verify_gguf_magic(&path).is_ok());
    }

    #[test]
    fn verify_gguf_magic_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.gguf");
        std::fs::write(&path, b"NOT a GGUF file").unwrap();
        assert!(verify_gguf_magic(&path).is_err());
    }

    #[test]
    fn verify_sha256_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(&path, expected).is_ok());
    }

    #[test]
    fn verify_sha256_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();
        let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(verify_sha256(&path, wrong).is_err());
    }

    #[test]
    fn verify_file_size_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"12345").unwrap();
        assert!(verify_file_size(&path, 5).is_ok());
    }

    #[test]
    fn verify_file_size_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"12345").unwrap();
        assert!(verify_file_size(&path, 10).is_err());
    }
}
