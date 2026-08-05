//! Print what the application knows about `shm-bridge.exe`.
//!
//! Three pieces have to agree about a frame — the application, the panel, and
//! the bridge — and the bridge was the one that could not be inspected. Every
//! failure that cost an evening looked identical from the driver's seat: the
//! panel saying "waiting for AC Pro Engineer" with `/dev/shm` holding the file,
//! at the right size, with the application running. A bridge built before the
//! frame grew maps too few bytes and CSP silently refuses to open the mapping.
//!
//! Run it with the bridge running to see which one is serving the game:
//!
//! ```text
//! cargo run -p ac_core --example bridge_probe
//! ```
//!
//! On Windows there is no bridge — the application creates the named mapping
//! itself — and this says so rather than reporting a missing component.

use ac_core::overlay::bridge::{self, BridgeStatus};
use ac_core::overlay::frame::{OVERLAY_MMF_NAME, OVERLAY_VERSION, OverlayFrame};
use ac_core::updater::CURRENT_VERSION;

fn main() {
    println!("AC Pro Engineer — bridge probe\n");

    println!("this application");
    println!("  release          {CURRENT_VERSION}");
    println!("  frame version    {OVERLAY_VERSION}");
    println!("  frame size       {} bytes", size_of::<OverlayFrame>());
    println!("  mapping          {OVERLAY_MMF_NAME}");
    println!("  bridge protocol  {}\n", bridge::BRIDGE_PROTOCOL);

    println!("shm-bridge.exe on disk");
    match bridge::installed_executable() {
        Some(path) => {
            println!("  found            {}", path.display());
            match bridge::version_in_executable(&path) {
                Some(version) => println!("  version          {version}"),
                // A bridge built before the marker existed is, by that fact,
                // older than this check — which is itself the answer.
                None => println!(
                    "  version          unknown — built before the version marker, \
                     so it predates this check"
                ),
            }
        }
        None => println!(
            "  not found        looked beside this executable and in the working \
             directory"
        ),
    }

    println!("\nshm-bridge.exe running");
    println!("  announced in     {}", bridge::info_path().display());
    let status = bridge::status(CURRENT_VERSION);
    match &status {
        BridgeStatus::NotRequired => {
            println!("  not required     Windows maps this directly; there is no bridge");
        }
        BridgeStatus::NotRunning => {
            println!("  not running      nothing has announced itself");
            println!(
                "\n  Start it in the game's Proton prefix:\n    \
                 protontricks-launch --appid 244210 shm-bridge.exe"
            );
        }
        BridgeStatus::Unannounced => {
            println!("  no announcement  but AC's pages are mapped, so a bridge is running");
            println!(
                "\n  TOO OLD          it predates the announcement, and every bridge that\n\
                 \x20                  old maps AC's four pages and nothing else — no overlay\n\
                 \x20                  mapping is ever created, so the panel waits forever.\n\
                 \x20                  Build one from this checkout:\n    \
                 cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu"
            );
        }
        BridgeStatus::Unreadable(why) => println!("  unreadable       {why}"),
        BridgeStatus::Incompatible { info, complaint } => {
            describe(info);
            println!("\n  INCOMPATIBLE     {}", complaint.describe());
            println!(
                "  The panel will wait forever. Fetch the published bridge with [B] \
                 on the overlay card, or rebuild it:\n    \
                 cargo build --release -p shm-bridge --target x86_64-pc-windows-gnu"
            );
        }
        BridgeStatus::Behind {
            info,
            expected_version,
        } => {
            describe(info);
            println!(
                "\n  BEHIND           this application is v{expected_version}. \
                 The frame still fits, so nothing is broken."
            );
        }
        BridgeStatus::Current(info) => {
            describe(info);
            println!("\n  CURRENT          same release as this application");
        }
    }

    // "Not workable" covers two different problems and one remedy each, so the
    // summary names which one rather than sending everyone to rebuild.
    println!(
        "\noverlay {}",
        match &status {
            _ if status.is_workable() => "can work as things stand",
            BridgeStatus::NotRunning => "cannot work until the bridge is started",
            BridgeStatus::Unannounced | BridgeStatus::Incompatible { .. } =>
                "cannot work until the bridge is replaced",
            _ => "cannot work; the bridge could not be identified",
        }
    );
}

fn describe(info: &bridge::BridgeInfo) {
    println!("  version          {}", info.version);
    println!("  bridge protocol  {}", info.protocol);
    println!("  maps             {} bytes", info.frame_bytes);
    println!("  as               {}", info.mmf);
    println!("  wine pid         {}", info.pid);
}
