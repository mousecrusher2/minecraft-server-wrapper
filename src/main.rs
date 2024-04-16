mod backup;

use clap::Parser;
use regex::Regex;
use std::path::PathBuf;
use tokio::{
    fs,
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::Command,
    time,
};

const COMMAND_PREFIX: &str = "!";

#[derive(Parser)]
#[command(version)]
struct Cli {
    server_options: Vec<String>,

    #[arg(long = "server-path", default_value = "./bedrock_server", short = 'p')]
    server_path: PathBuf,

    #[arg(long = "backup-interval", short = 'i')]
    backup_interval: Option<humantime::Duration>,

    #[arg(long = "backup-folder", default_value = "./backups", short = 'f')]
    backup_folder: PathBuf,

    #[arg(long = "backup-count", short = 'c')]
    backup_count: Option<usize>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let mut child = Command::new(&cli.server_path)
        .env("LD_LIBRARY_PATH", cli.server_path.parent().unwrap())
        .args(cli.server_options)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .current_dir(cli.server_path.parent().unwrap())
        .spawn()
        .expect("Failed to start server");

    time::sleep(time::Duration::from_secs(3)).await;

    if !cli.backup_folder.exists() {
        fs::create_dir_all(&cli.backup_folder)
            .await
            .expect("Failed to create backup folder");
    }

    if !cli.backup_folder.is_dir() {
        eprintln!("Backup folder must be a directory");
        std::process::exit(1);
    }

    let mut self_stdin = BufReader::new(io::stdin()).lines();
    let mut self_stdout = BufWriter::new(io::stdout());
    let mut server_stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut server_stdin = child.stdin.take().unwrap();

    let backuper = backup::Backuper::new(
        cli.backup_folder,
        cli.server_path.parent().unwrap().to_path_buf().join("worlds"),
        cli.backup_count.unwrap_or(1),
    );

    let mut backup_interval = time::interval(
        cli.backup_interval
            .unwrap_or_else(|| humantime::parse_duration("1y").unwrap().into())
            .into(),
    );
    backup_interval.tick().await;

    let command_re = Regex::new(r" *save( +\S*)?").unwrap();
    let backup_command_re = Regex::new(r"backup *").unwrap();
    let ready_backups_re =
        Regex::new(r"\[.+\] Data saved. Files are now ready to be copied.").unwrap();

    loop {
        tokio::select! {
            biased;
            res = child.wait() => {
                eprintln!("Server exited");
                res.expect("Failed to wait for server");
                std::process::exit(0);
            }

            line = server_stdout.next_line() => {
                if let Some(line) = line.expect("Failed to read from server stdout") {
                    self_stdout.write_all(line.as_bytes()).await.expect("Failed to write to stdout");
                    self_stdout.write_u8(b'\n').await.expect("Failed to write to stdout");
                    self_stdout.flush().await.unwrap();
                }
            }

            line = self_stdin.next_line() => {
                let line = line
                    .expect("Failed to read from stdin")
                    .expect("stdin closed. Don't you redirect stdin?");
                if line.starts_with(COMMAND_PREFIX) {
                    if backup_command_re.is_match(line.strip_prefix(COMMAND_PREFIX).unwrap()) {
                        self_stdout.write_all(b"Backup started\n").await.expect("Failed to write to stdout");
                        self_stdout.flush().await.expect("Failed to flush stdout");
                        server_stdin.write_all(b"save hold\n").await.expect("Failed to write to server stdin");
                        server_stdin.write_all(b"save query\n").await.expect("Failed to write to server stdin");
                        server_stdin.flush().await.expect("Failed to flush server stdin");

                        server_stdout.next_line().await.expect("Failed to read from server stdout");
                        loop {
                            let line = server_stdout.next_line().await
                                .expect("Failed to read from server stdout")
                                .expect("Server stdout closed");
                            if ready_backups_re.is_match(&line) {
                                break;
                            }
                            self_stdout.write_all(line.as_bytes()).await.expect("Failed to write to stdout");
                            self_stdout.write_u8(b'\n').await.expect("Failed to write to stdout");
                            self_stdout.flush().await.expect("Failed to flush stdout");
                            server_stdin.write_all(b"save query\n").await.expect("Failed to write to server stdin");
                            server_stdin.flush().await.expect("Failed to flush server stdin");
                            tokio::time::sleep(time::Duration::from_millis(300)).await;
                        }
                        let line = server_stdout.next_line().await
                            .expect("Failed to read from server stdout")
                            .expect("Server stdout closed");
                        backuper.backup(line).await;
                        server_stdin.write_all(b"save resume\n").await.expect("Failed to write to server stdin");
                        server_stdin.flush().await.expect("Failed to flush server stdin");

                        self_stdout.write_all(b"Backup finished").await.expect("Failed to write to stdout");
                        self_stdout.flush().await.expect("Failed to flush stdout");

                    } else {
                        self_stdout.write_all(b"Unknown command").await.expect("Failed to write to stdout");
                        self_stdout.flush().await.expect("Failed to flush stdout");
                    }
                } else if command_re.is_match(&line) {
                    self_stdout.write_all(b"Don't use \"save xxxx\" command").await.expect("Failed to write to stdout");
                    self_stdout.flush().await.expect("Failed to flush stdout");
                } else {
                    server_stdin.write_all(line.as_bytes()).await.expect("Failed to write to server stdin");
                    server_stdin.write_u8(b'\n').await.expect("Failed to write to server stdin");
                    server_stdin.flush().await.expect("Failed to flush server stdin");
                }
            }

            _ = backup_interval.tick() => {
                self_stdout.write_all(b"Backup started\n").await.expect("Failed to write to stdout");
                self_stdout.flush().await.expect("Failed to flush stdout");
                server_stdin.write_all(b"save hold\n").await.expect("Failed to write to server stdin");
                server_stdin.write_all(b"save query\n").await.expect("Failed to write to server stdin");
                server_stdin.flush().await.expect("Failed to flush server stdin");

                server_stdout.next_line().await.expect("Failed to read from server stdout");

                loop {
                    let line = server_stdout.next_line().await
                        .expect("Failed to read from server stdout")
                        .expect("Server stdout closed");
                    if ready_backups_re.is_match(&line) {
                        break;
                    }
                    self_stdout.write_all(line.as_bytes()).await.expect("Failed to write to stdout");
                    self_stdout.write_u8(b'\n').await.expect("Failed to write to stdout");
                    self_stdout.flush().await.expect("Failed to flush stdout");

                    server_stdin.write_all(b"save query\n").await.expect("Failed to write to server stdin");
                    server_stdin.flush().await.expect("Failed to flush server stdin");
                }
                let line = server_stdout.next_line().await
                    .expect("Failed to read from server stdout")
                    .expect("Server stdout closed");
                backuper.backup(line).await;
                server_stdin.write_all(b"save resume\n").await.expect("Failed to write to server stdin");
                server_stdin.flush().await.expect("Failed to flush server stdin");

                self_stdout.write_all(b"Backup finished\n").await.expect("Failed to write to stdout");
                self_stdout.flush().await.expect("Failed to flush stdout");
            }

        }
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Local};

    #[test]
    fn format_chrono() {
        let now = Local::now();
        let formatted = now.format("%F %H_%M_%S%.f %z").to_string();
        assert_eq!(now, DateTime::parse_from_str(&formatted, "%F %H_%M_%S%.f %z").unwrap());
    }
}
