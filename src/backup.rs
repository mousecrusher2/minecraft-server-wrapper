use chrono::{DateTime, Local};
use regex::Regex;
use std::path::PathBuf;
use tokio::fs;
use tokio_stream::{wrappers::ReadDirStream, StreamExt};

pub struct Backuper {
    backup_folder: PathBuf,
    origin_folder: PathBuf,
    backup_count: usize,
}

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
        let origin_world_folder = self.origin_folder.join(world_name);
        let backup_world_folder = self.backup_folder.join(world_name);
        let mut t = ReadDirStream::new(fs::read_dir(origin_world_folder).await.unwrap());
        let mut v = Vec::new();
        while let Some(entry) = t.next().await {
            let entry = entry.unwrap();
            if !entry.file_type().await.unwrap().is_dir() {
                continue;
            }
            if let Ok(d) =
                DateTime::parse_from_str(entry.file_name().to_str().unwrap(), "%F %H_%M_%S%.f %z")
            {
                v.push((entry, d));
            }
        }
        v.sort_by_key(|(_, d)| *d);
        let ndel = v.len().saturating_sub(self.backup_count.saturating_sub(1));
        let mut tasks = Vec::new();
        for (entry, _) in v.into_iter().take(ndel) {
            tasks.push(tokio::spawn(async move {
                fs::remove_dir_all(entry.path())
                    .await
                    .expect("Failed to remove directory");
            }));
        }
        let target_backup_folder =
            backup_world_folder.join(Local::now().format("%F %H_%M_%S%.f %z").to_string());
        fs::create_dir_all(&target_backup_folder)
            .await
            .expect("Failed to create backup directory");
        let files = self.parse_backup_files(file_paths);
        for (path, size) in files {
            let origin_path = self.origin_folder.join(&path);
            let target_path = target_backup_folder.join(&path);
            tasks.push(tokio::spawn(async move {
                fs::create_dir_all(target_path.parent().unwrap())
                    .await
                    .expect("Failed to create parent directories");
                fs::copy(origin_path, &target_path)
                    .await
                    .expect("Failed to copy file");
                fs::OpenOptions::new()
                    .write(true)
                    .open(target_path)
                    .await
                    .expect("Failed to open file")
                    .set_len(size)
                    .await
                    .expect("Failed to set file size");
            }));
        }
        for task in tasks {
            task.await.expect("Failed to wait for task");
        }
    }
}
