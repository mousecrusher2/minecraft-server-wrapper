use chrono::{DateTime, Local};
use futures::{
    future::FutureExt,
    stream::{self, StreamExt},
};
use regex::Regex;
use std::path::PathBuf;
use tokio::{fs, io::AsyncWriteExt};
use tokio_stream::wrappers::ReadDirStream;

pub struct Backuper {
    backup_folder: PathBuf,
    origin_folder: PathBuf,
    backup_count: usize,
}

const PARALLEL_LIMIT: usize = 100;

impl Backuper {
    pub(super) fn new(backup_folder: PathBuf, origin_folder: PathBuf, backup_count: usize) -> Self {
        if !backup_folder.is_dir() {
            panic!("Backup folder must be a directory");
        }
        if !origin_folder.is_dir() {
            panic!("Origin folder must be a directory");
        }
        Self {
            backup_folder,
            origin_folder,
            backup_count,
        }
    }

    fn parse_backup_files(&self, file_paths: String) -> Vec<(String, u64)> {
        thread_local! {
            static BACKUP_FILE_RE: Regex = Regex::new(r" (?<path>\S+):(?<size>[1-9][0-9]*)").unwrap();
        }
        BACKUP_FILE_RE.with(|re| {
            re.captures_iter(&file_paths)
                .map(|cap| {
                    let (_, [path, size]) = cap.extract();
                    (
                        path.to_string(),
                        size.parse().expect("Failed to parse size"),
                    )
                })
                .collect()
        })
    }

    // Backup folder is like: ./backups/world_name/2021-08-01 12_34_56+0000
    pub(super) async fn backup(&self, file_paths: String) {
        let world_name = file_paths.split('/').next().unwrap();
        let backup_world_folder = self.backup_folder.join(world_name);
        fs::create_dir(&backup_world_folder)
            .await
            .or_else(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => Ok(()),
                _ => Err(e),
            })
            .expect("Unknown error occurred while creating backup folder");
        let task1 = ReadDirStream::new(fs::read_dir(&backup_world_folder).await.unwrap())
            .filter_map(|entry| async move {
                let entry = entry.unwrap();
                if !entry.file_type().await.unwrap().is_dir() {
                    return None;
                }
                let d = DateTime::parse_from_str(
                    entry.file_name().to_str().unwrap(),
                    "%F %H_%M_%S%.f %z",
                )
                .ok()?;
                Some((entry, d))
            })
            .collect::<Vec<_>>()
            .then(|mut v| {
                v.sort_by_key(|(_, d)| *d);
                let ndel = v.len().saturating_sub(self.backup_count.saturating_sub(1));
                stream::iter(v)
                    .take(ndel)
                    .for_each_concurrent(None, |(entry, _)| async move {
                        fs::remove_dir_all(entry.path())
                            .await
                            .expect("Failed to remove directory");
                    })
            });
        let target_backup_folder =
            backup_world_folder.join(Local::now().format("%F %H_%M_%S%.f %z").to_string());
        let files = self.parse_backup_files(file_paths);
        let task2 = fs::create_dir_all(&target_backup_folder).then(|r| {
            r.expect("Failed to create backup folder");
            stream::iter(files).for_each_concurrent(PARALLEL_LIMIT, |(path, size)| {
                let origin_path = self.origin_folder.join(&path);
                let target_path = target_backup_folder.join(&path);
                async move {
                    fs::create_dir_all(target_path.parent().unwrap())
                        .await
                        .unwrap();
                    fs::copy(origin_path, &target_path).await.unwrap();
                    let mut f = fs::OpenOptions::new()
                        .write(true)
                        .open(target_path)
                        .await
                        .expect("Failed to open file");
                    f.set_len(size).await.expect("Failed to set file size");
                    f.flush().await.expect("Failed to flush file");
                }
            })
        });
        tokio::join!(task1, task2);
    }
}
