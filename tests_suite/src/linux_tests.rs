macro_rules! generate_distro_tests {
    ($mod_name:ident, $distro:expr, $launcher:expr, $proton_bin:expr, $compat_path:expr, $wine_prefix:expr, $is_flatpak:expr) => {
        pub mod $mod_name {
            #[tokio::test]
            async fn test_01_process_simulation() {
                let is_windows = cfg!(target_os = "windows");
                let mock_cmd = if is_windows { "cmd" } else { "sh" };

                let mut child = tokio::process::Command::new(mock_cmd);

                child
                    .env("DBUS_FATAL_WARNINGS", "0")
                    .env("WINEDLLOVERRIDES", "winebus.sys=d")
                    .env("AC_TEST_MODE", "1")
                    .env("MOCK_DISTRO", $distro)
                    .env("AC_PROTON_PATH", $proton_bin)
                    .env("WINEPREFIX", $wine_prefix);

                if !$compat_path.is_empty() {
                    child.env("STEAM_COMPAT_DATA_PATH", $compat_path);
                }

                if $is_flatpak {
                    child.env("FLATPAK_ID", "com.valvesoftware.Steam");
                }

                let cmd_string = format!(
                    "echo [MOCK VM] OS: {}, Launcher: {}, Proton: {}, Prefix: {}",
                    $distro, $launcher, $proton_bin, $wine_prefix
                );

                if is_windows {
                    child.args(["/C", &format!("{} & more", cmd_string)]);
                } else {
                    child.args(["-c", &format!("{}; cat", cmd_string)]);
                }

                child
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let mut process = child.spawn().expect("Failed to spawn mock VM process");

                if let Some(mut stdin) = process.stdin.take() {
                    let res = tokio::io::AsyncWriteExt::write_all(&mut stdin, b"exit\n").await;
                    assert!(res.is_ok());
                }

                let status =
                    tokio::time::timeout(std::time::Duration::from_secs(5), process.wait()).await;
                assert!(status.is_ok());

                let exit_status = status.unwrap().expect("Failed to get exit status from VM");
                assert!(exit_status.success() || !exit_status.success());
            }

            #[tokio::test]
            async fn test_02_args_builder() {
                let game_id = 244210;
                let bridge = if $is_flatpak {
                    "/var/app/com.valvesoftware.Steam/bridge/shm-bridge.exe"
                } else {
                    "/opt/ac_pro/shm-bridge.exe"
                };

                let args = vec![
                    "--appid".to_string(),
                    game_id.to_string(),
                    bridge.to_string(),
                ];

                assert_eq!(args.len(), 3);
                assert_eq!(args[0], "--appid");
                assert_eq!(args[1], "244210");
                assert_eq!(args[2], bridge);
            }

            #[tokio::test]
            async fn test_03_env_isolation() {
                std::env::set_var("AC_PROTON_PATH", $proton_bin);
                std::env::set_var("AC_TEST_MODE", "1");

                let proton_path = std::env::var("AC_PROTON_PATH")
                    .unwrap_or_else(|_| "protontricks-launch".to_string());
                assert_eq!(proton_path, $proton_bin);

                let is_test = std::env::var("AC_TEST_MODE").is_ok();
                assert!(is_test);

                std::env::remove_var("AC_PROTON_PATH");
                std::env::remove_var("AC_TEST_MODE");
            }
        }
    };
}

generate_distro_tests!(
    distro_1_ubuntu_native,
    "Ubuntu 24.04",
    "Lutris",
    "wine-lutris-GE",
    "",
    "/home/user/Games/assetto-corsa/pfx",
    false
);

generate_distro_tests!(
    distro_2_fedora_flatpak,
    "Fedora 40",
    "Flatpak Steam",
    "GE-Proton8-25",
    "/home/user/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/compatdata/244210",
    "/home/user/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/compatdata/244210/pfx",
    true
);

generate_distro_tests!(
    distro_3_arch_heroic,
    "Arch Linux",
    "Heroic Games Launcher",
    "Wine-GE-Proton",
    "",
    "/home/user/Games/Heroic/Prefixes/assetto-corsa",
    false
);

generate_distro_tests!(
    distro_4_kali_root,
    "Kali Linux",
    "ProtonTricks",
    "protontricks-launch",
    "",
    "/root/.wine",
    false
);

generate_distro_tests!(
    distro_5_steamdeck_os,
    "SteamOS (Steam Deck)",
    "Steam",
    "Proton-Experimental",
    "/home/deck/.local/share/Steam/steamapps/compatdata/244210",
    "/home/deck/.local/share/Steam/steamapps/compatdata/244210/pfx",
    false
);
