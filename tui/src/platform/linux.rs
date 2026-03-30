use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot::Sender;
use tokio::task::{JoinHandle, block_in_place};
use tracing::{error, info};

static GAME_ID: u32 = 244210;

/// This is a helper struct to start a `Shared Memory Bridge` (`shm-bridge.exe`) process in Proton.
///
/// [protontricks](https://github.com/Matoking/protontricks) is required to launch the bridge
pub struct SharedMemoryBridge {
    handle: Option<JoinHandle<Result<(), anyhow::Error>>>,
    exit_tx: Option<Sender<()>>,
}

impl SharedMemoryBridge {
    pub async fn start() -> Result<Self, std::io::Error> {
        info!("[shm-bridge] Starting memory bridge process...");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let pwd = match std::env::current_dir()?.as_os_str().to_str() {
            Some(pwd) => pwd.to_string(),
            None => ".".to_string(),
        };

        let process_handle = tokio::spawn(async move {
            info!("[shm-bridge] Starting bridge process from");

            let proton_cmd = std::env::var("AC_PROTON_PATH")
                .unwrap_or_else(|_| "protontricks-launch".to_string());
            let is_test = std::env::var("AC_TEST_MODE").is_ok();

            let mut child = if is_test {
                if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.args(["/C", "echo Simulated Proton Execution Started & more"]);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.args(["-c", "echo Simulated Proton Execution Started; cat"]);
                    c
                }
            } else {
                let mut c = Command::new(proton_cmd);
                c.args([
                    "--appid",
                    &GAME_ID.to_string(),
                    &format!("{pwd}/shm-bridge.exe"),
                ]);
                c
            };

            let mut child = child
                .envs(std::env::vars())
                // These envs are required to fix 100% CPU usage by winedevice.exe
                .env("DBUS_FATAL_WARNINGS", "0")
                .env("WINEDLLOVERRIDES", "winebus.sys=d")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;

            let stdout = child.stdout.take();
            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout).lines();
                tokio::task::spawn(async move {
                    while let Ok(Some(line)) = reader.next_line().await {
                        info!("[shm-bridge/out] {line}");
                    }
                });
            }

            let stderr = child.stderr.take();
            if let Some(stderr) = stderr {
                let mut reader = BufReader::new(stderr).lines();
                tokio::task::spawn(async move {
                    while let Ok(Some(line)) = reader.next_line().await {
                        info!("[shm-bridge/err] {line}");
                    }
                });
            }

            let input = child.stdin.take();
            if let Some(mut input) = input {
                let should_exit = rx.await.is_ok();
                if should_exit {
                    info!("[shm-bridge] Exiting bridge process...");
                    // Send an 'exit' command to the bridge process.
                    // This is not the best way to stop it, but the easiest to make it work
                    // through Protontricks layer. We cannot send any signal to the bridge
                    // process, because it runs in Wine, and we cannot just kill Protontricks,
                    // or there will be remaining memory files links in /dev/shm.
                    input.write_all("exit\n".as_bytes()).await?;
                    input.flush().await?;
                }
            }

            let status = child.wait().await?;
            info!("[shm-bridge] Bridge process exited, {}", status);
            Ok::<(), anyhow::Error>(())
        });

        Ok(Self {
            handle: Some(process_handle),
            exit_tx: Some(tx),
        })
    }
}

impl Drop for SharedMemoryBridge {
    fn drop(&mut self) {
        info!("[shm-bridge] Shutting down memory bridge process...");
        if let Some(tx) = self.exit_tx.take() {
            let _unused = tx.send(());
        }

        if let Some(handle) = self.handle.take() {
            let result = block_in_place(move || tokio::runtime::Handle::current().block_on(handle));
            if let Err(e) = result {
                error!("[shm-bridge] Failed to join bridge process handle: {:?}", e);
            }
        }
        info!("[shm-bridge] Memory bridge process finished...");
    }
}
