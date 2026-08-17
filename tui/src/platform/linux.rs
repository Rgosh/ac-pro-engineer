use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot::Sender;
use tokio::task::{JoinHandle, block_in_place};
use tokio::time::timeout;
use tracing::{error, info};

/// How long to wait for the bridge to acknowledge the exit request before
/// giving up on it. Long enough for a Wine process to wind down, short enough
/// that quitting the app never feels hung.
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Which Proton prefix the bridge is started in.
///
/// It used to be Assetto Corsa's appid, written down here as a constant, which
/// is the same as saying this program reads one game. A bridge started in the
/// wrong prefix creates the mappings and the game never writes into them, and
/// every symptom of that — no telemetry, a panel waiting forever, a launcher
/// stuck on "waiting for the simulator" — looks like something else entirely.
///
/// It cannot be worked out from what is running, either: the bridge is what
/// *makes* telemetry reachable, so waiting for telemetry would be a circle,
/// and the bridge has to exist before the game asks for the mappings — which
/// is before there is a process to detect. So it is the game the driver chose
/// on the launcher, and choosing another one restarts the bridge.
pub fn prefix_of(game: &ac_core::games::Game) -> u32 {
    game.backend()
        .map(|backend| backend.app_id)
        .unwrap_or_default()
}

/// This is a helper struct to start a `Shared Memory Bridge` (`shm-bridge.exe`) process in Proton.
///
/// [protontricks](https://github.com/Matoking/protontricks) is required to launch the bridge
pub struct SharedMemoryBridge {
    handle: Option<JoinHandle<Result<(), anyhow::Error>>>,
    exit_tx: Option<Sender<()>>,
}

/// Locate `shm-bridge.exe`.
///
/// The same search the launcher's overlay card and `bridge_probe` use, so the
/// bridge the card *judges* is the bridge the application *spawns*. They used
/// to be two separate functions looking in different places: this one knew
/// nothing about `target/x86_64-pc-windows-gnu/release/`, so a checkout that
/// had just cross-compiled a bridge still had to have a copy at the root of
/// the repository before `cargo run` would start one — and the card, which did
/// know about the build target, could report a version the application had
/// never launched.
///
/// Falls back to the working directory when there is no bridge anywhere, so
/// the spawn fails naming a path a person can act on rather than an empty one.
fn bridge_path() -> std::path::PathBuf {
    ac_core::overlay::bridge::installed_executable().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(ac_core::overlay::bridge::BRIDGE_EXE)
    })
}

impl SharedMemoryBridge {
    /// Start the bridge inside one game's Proton prefix.
    ///
    /// `app_id` is Steam's number for the game the driver chose — see
    /// [`prefix_of`]. It is an argument rather than a constant because the two
    /// games this build reads publish into two different prefixes, and the
    /// bridge can only be in one of them.
    pub async fn start(app_id: u32) -> Result<Self, std::io::Error> {
        info!("[shm-bridge] Starting memory bridge process in prefix {app_id}...");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let pwd = match bridge_path().to_str() {
            Some(pwd) => pwd.to_string(),
            None => ".".to_string(),
        };

        let proton_cmd =
            std::env::var("AC_PROTON_PATH").unwrap_or_else(|_| "protontricks-launch".to_string());
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
            c.args(["--appid", &app_id.to_string(), &pwd]);
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

        let process_handle = tokio::spawn(async move {
            info!("[shm-bridge] Starting bridge process from");
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
                    //
                    // Failing to deliver it is logged rather than propagated:
                    // `?` here skipped the `child.wait()` below and leaked the
                    // process, which is a worse outcome than a bridge that did
                    // not hear the request.
                    if let Err(error) = input.write_all("exit\n".as_bytes()).await {
                        error!("[shm-bridge] Could not send the exit command: {error}");
                    } else if let Err(error) = input.flush().await {
                        error!("[shm-bridge] Could not flush the exit command: {error}");
                    }
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
            // Bounded. The bridge only shuts down when it reads "exit" on
            // stdin, so a bridge that has already died, or a Wine layer that
            // is not passing stdin through, left this blocking forever — the
            // app would simply never finish quitting, with nothing on screen
            // to say why.
            let result = block_in_place(move || {
                tokio::runtime::Handle::current()
                    .block_on(async { timeout(SHUTDOWN_TIMEOUT, handle).await })
            });

            match result {
                Err(_elapsed) => error!(
                    "[shm-bridge] Bridge did not exit within {:?}; abandoning it. \
                     Stale mappings may remain in /dev/shm.",
                    SHUTDOWN_TIMEOUT
                ),
                Ok(Err(join_error)) => {
                    error!("[shm-bridge] Failed to join bridge process handle: {join_error:?}")
                }
                // The inner Result used to be discarded, so a failure inside
                // the task -- writing the exit command, or waiting on the
                // child -- disappeared without trace.
                Ok(Ok(Err(task_error))) => {
                    error!("[shm-bridge] Bridge task failed: {task_error:?}")
                }
                Ok(Ok(Ok(()))) => {}
            }
        }
        info!("[shm-bridge] Memory bridge process finished...");
    }
}
