//! Файлы пользователей на локальном диске. Один VPS, один каталог
//! (`FILES_DIR`) — S3 появится заменой этого модуля, когда серверов станет
//! больше одного.
//!
//! Всё через `tokio::fs`: в хендлере нельзя блокировать поток исполнителя.
//! Имя файла собирает домен из id записи (`<uuid>.<ext>`), поэтому в имя не
//! попадает ничего от пользователя — путь наружу увести нечем.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

fn path_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

/// Записать файл. Каталог создаётся при первой записи: в проде это volume,
/// в тестах — временный каталог, и обоим не нужен отдельный шаг подготовки.
pub async fn save(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    let mut file = tokio::fs::File::create(path_of(dir, name)).await?;
    file.write_all(bytes).await?;
    // без явного flush часть буфера могла бы остаться незаписанной
    file.flush().await
}

/// Прочитать файл целиком: размер ограничен на приёме, в память влезает.
pub async fn read(dir: &Path, name: &str) -> std::io::Result<Vec<u8>> {
    tokio::fs::read(path_of(dir, name)).await
}

/// Убрать файл после сбоя, лучшее из возможного: файла может уже не быть
/// (это не ошибка), а прочие проблемы уходят в лог — падать здесь не из-за
/// чего, запись в базе всё равно не появилась.
pub async fn remove(dir: &Path, name: &str) {
    if let Err(err) = tokio::fs::remove_file(path_of(dir, name)).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(name, error = %err, "осиротевший файл не удалился");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saved_file_reads_back() {
        let dir = std::env::temp_dir().join(format!("storage-test-{}", uuid::Uuid::new_v4()));
        save(&dir, "a.bin", b"hello").await.unwrap();
        assert_eq!(read(&dir, "a.bin").await.unwrap(), b"hello");
        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn missing_file_is_an_error_not_empty_bytes() {
        let dir = std::env::temp_dir().join("storage-test-missing");
        assert!(read(&dir, "no-such-file").await.is_err());
    }
}
