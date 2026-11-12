//! Wheel and button-box bindings: which button does what, set by pressing it.
//!
//! **The game binds a button as a device and a number.** `controls.ini` has
//! a section per action — `[GEARUP]`, `[DRS]`, `[BALANCEUP]` — with `JOY`,
//! the device's place in `[CONTROLLERS]`, and `BUTTON`, its number on that
//! device. `[CONTROLLERS]` names each device (`CON0=`) and identifies it by
//! its DirectInput product (`PGUID0=`), so a wheel keeps its bindings when
//! it is plugged into another port. An H-pattern shifter is one section,
//! `[SHIFTER]`, with a key per gear. The section names are the game's and
//! Content Manager's: a launcher that writes another name binds nothing.
//!
//! **A device is matched by its product, not its place.** What a launcher
//! sees of a wheel is its USB vendor and product; DirectInput's product
//! identity is built from exactly those two numbers ([`product_guid`]), on
//! Windows and under Proton alike. So a button pressed here finds the right
//! `CON` entry whatever order the game enumerates devices in, and a device
//! the file does not know yet is added at the end.
//!
//! **Buttons only.** A button's number is the same to the system's joystick
//! interface and to DirectInput; an axis's is not — Linux numbers the axes a
//! device has, DirectInput numbers the axes it could have — so pedals and
//! the wheel stay the game's own calibration screen's job.

use super::ini;
use std::path::{Path, PathBuf};

/// One thing a button can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    /// The section in `controls.ini`.
    pub section: &'static str,
    /// The key holding the button: `BUTTON`, or a gear under `[SHIFTER]`.
    pub key: &'static str,
    /// What a driver calls it.
    pub label: &'static str,
    /// Which group it is listed under.
    pub group: &'static str,
}

const fn button(section: &'static str, label: &'static str, group: &'static str) -> Action {
    Action {
        section,
        key: "BUTTON",
        label,
        group,
    }
}

const fn gear(key: &'static str, label: &'static str) -> Action {
    Action {
        section: "SHIFTER",
        key,
        label,
        group: "H-pattern shifter",
    }
}

/// Every action the game binds to a button, in the order they are listed.
pub const ACTIONS: &[Action] = &[
    button("GEARUP", "Next gear", "Gears"),
    button("GEARDN", "Previous gear", "Gears"),
    button("DRS", "DRS", "Car"),
    button("KERS", "KERS", "Car"),
    button("ACTION_HEADLIGHTS", "Headlights", "Car"),
    button("ACTION_HEADLIGHTS_FLASH", "Flash headlights", "Car"),
    button("ACTION_HORN", "Horn", "Car"),
    button("BALANCEUP", "Brake bias forward", "Brake bias"),
    button("BALANCEDN", "Brake bias rearward", "Brake bias"),
    button("TCUP", "Traction control up", "Electronics"),
    button("TCDN", "Traction control down", "Electronics"),
    button("ABSUP", "ABS up", "Electronics"),
    button("ABSDN", "ABS down", "Electronics"),
    button("ENGINE_BRAKE_UP", "Engine brake up", "Electronics"),
    button("ENGINE_BRAKE_DN", "Engine brake down", "Electronics"),
    button("TURBOUP", "Turbo up", "Electronics"),
    button("TURBODN", "Turbo down", "Electronics"),
    button("MGUK_DELIVERY_UP", "MGU-K delivery up", "Hybrid"),
    button("MGUK_DELIVERY_DN", "MGU-K delivery down", "Hybrid"),
    button("MGUK_RECOVERY_UP", "MGU-K recovery up", "Hybrid"),
    button("MGUK_RECOVERY_DN", "MGU-K recovery down", "Hybrid"),
    button("MGUH_MODE", "MGU-H mode", "Hybrid"),
    button("GLANCELEFT", "Look left", "View"),
    button("GLANCERIGHT", "Look right", "View"),
    button("GLANCEBACK", "Look back", "View"),
    button("ACTION_CHANGE_CAMERA", "Change camera", "View"),
    gear("GEAR_1", "First"),
    gear("GEAR_2", "Second"),
    gear("GEAR_3", "Third"),
    gear("GEAR_4", "Fourth"),
    gear("GEAR_5", "Fifth"),
    gear("GEAR_6", "Sixth"),
    gear("GEAR_7", "Seventh"),
    gear("GEAR_R", "Reverse"),
];

/// The groups, in the order they are listed.
pub fn groups() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for action in ACTIONS {
        if !out.contains(&action.group) {
            out.push(action.group);
        }
    }
    out
}

/// DirectInput's product identity for a USB vendor and product, as the game
/// and Content Manager write it in `PGUID`.
pub fn product_guid(vendor: u16, product: u16) -> String {
    format!("{product:04X}{vendor:04X}-0000-0000-0000-504944564944")
}

/// One device the file knows, at its place in `[CONTROLLERS]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controller {
    pub index: u32,
    pub name: String,
    /// `PGUID`, when written.
    pub product: Option<String>,
}

/// The devices `[CONTROLLERS]` lists, in order.
pub fn controllers(text: &str) -> Vec<Controller> {
    (0..32)
        .map_while(|index| {
            let name = ini::value(text, "CONTROLLERS", &format!("CON{index}"))?;
            Some(Controller {
                index,
                name,
                product: ini::value(text, "CONTROLLERS", &format!("PGUID{index}"))
                    .filter(|p| !p.is_empty() && p != "0"),
            })
        })
        .collect()
}

fn same_product(a: &str, b: &str) -> bool {
    let bare = |s: &str| s.trim_matches(['{', '}']).to_ascii_uppercase();
    bare(a) == bare(b)
}

/// A button as pressed: on which device, and its number there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pressed {
    /// The device's name, as the system reports it.
    pub device: String,
    /// Its USB vendor and product, when known.
    pub ids: Option<(u16, u16)>,
    pub button: u32,
}

/// What an action is bound to now: the device and the button.
pub fn bound(text: &str, action: &Action) -> Option<(Controller, u32)> {
    let joy: i64 = ini::value(text, action.section, "JOY")?.parse().ok()?;
    let button: i64 = ini::value(text, action.section, action.key)?.parse().ok()?;
    if joy < 0 || button < 0 {
        return None;
    }
    let controller = controllers(text)
        .into_iter()
        .find(|c| i64::from(c.index) == joy)
        .unwrap_or(Controller {
            index: joy as u32,
            name: format!("device {joy}"),
            product: None,
        });
    Some((controller, button as u32))
}

/// The actions already on this button of this device, but `except`.
pub fn taken_by(text: &str, pressed: &Pressed, except: &Action) -> Vec<Action> {
    let product = pressed.ids.map(|(v, p)| product_guid(v, p));
    let Some(index) = find_controller(&controllers(text), &pressed.device, product.as_deref())
    else {
        return Vec::new();
    };
    ACTIONS
        .iter()
        .filter(|action| *action != except)
        // A shifter's gears and a sequential paddle are different devices
        // more often than not; only the same section kind is compared.
        .filter(|action| (action.section == "SHIFTER") == (except.section == "SHIFTER"))
        .filter(|action| {
            bound(text, action).is_some_and(|(c, b)| c.index == index && b == pressed.button)
        })
        .copied()
        .collect()
}

fn find_controller(known: &[Controller], device: &str, product: Option<&str>) -> Option<u32> {
    known
        .iter()
        .find(|c| {
            product
                .zip(c.product.as_deref())
                .is_some_and(|(a, b)| same_product(a, b))
        })
        .or_else(|| {
            known
                .iter()
                .find(|c| c.name.trim().eq_ignore_ascii_case(device.trim()))
        })
        .map(|c| c.index)
}

/// The text with `action` bound to the button pressed — the device added to
/// `[CONTROLLERS]` when the file does not know it, every other line as it was.
pub fn bind(text: &str, action: &Action, pressed: &Pressed) -> String {
    let product = pressed.ids.map(|(v, p)| product_guid(v, p));
    bind_to(
        text,
        action,
        &pressed.device,
        product.as_deref(),
        pressed.button,
    )
}

fn bind_to(
    text: &str,
    action: &Action,
    device: &str,
    product: Option<&str>,
    button: u32,
) -> String {
    let known = controllers(text);
    let (mut out, index) = match find_controller(&known, device, product) {
        Some(index) => (text.to_string(), index),
        None => {
            let index = known.len() as u32;
            let mut out =
                ini::with_value(text, "CONTROLLERS", &format!("CON{index}"), device.trim());
            if let Some(product) = product {
                out = ini::with_value(&out, "CONTROLLERS", &format!("PGUID{index}"), product);
            }
            (out, index)
        }
    };
    out = ini::with_value(&out, action.section, "JOY", &index.to_string());
    out = ini::with_value(&out, action.section, action.key, &button.to_string());
    if action.section == "SHIFTER" {
        out = ini::with_value(&out, "SHIFTER", "ACTIVE", "1");
    }
    out
}

/// One action of a layout: what it is on, by the device's own identity
/// rather than its place — so a layout kept today binds the same buttons
/// after the devices are plugged in another order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Placed {
    pub section: String,
    pub key: String,
    pub device: String,
    #[serde(default)]
    pub product: Option<String>,
    pub button: u32,
}

/// Every button binding at once: a button box set for GT cars, another for
/// Formula cars.
pub type Layout = Vec<Placed>;

/// The layout the file holds now: every action that is bound.
pub fn layout(text: &str) -> Layout {
    ACTIONS
        .iter()
        .filter_map(|action| {
            let (controller, button) = bound(text, action)?;
            Some(Placed {
                section: action.section.to_string(),
                key: action.key.to_string(),
                device: controller.name,
                product: controller.product,
                button,
            })
        })
        .collect()
}

/// The text with a whole layout in force: every action it names bound,
/// every other action bound to nothing. Axes and every other line stay.
pub fn with_layout(text: &str, layout: &Layout) -> String {
    let mut out = text.to_string();
    for action in ACTIONS {
        let placed = layout
            .iter()
            .find(|p| p.section == action.section && p.key == action.key);
        out = match placed {
            Some(p) => bind_to(&out, action, &p.device, p.product.as_deref(), p.button),
            None if bound(&out, action).is_some() => unbind(&out, action),
            None => out,
        };
    }
    out
}

/// The text with `action` bound to nothing.
pub fn unbind(text: &str, action: &Action) -> String {
    let out = ini::with_value(text, action.section, action.key, "-1");
    if action.section == "SHIFTER" {
        // The shifter's device stays: other gears are still on it.
        return out;
    }
    ini::with_value(&out, action.section, "JOY", "-1")
}

/// Bind in the game's own `controls.ini`, after copying it aside into
/// `backups` — a binding is a driver's evening of setting up, and a wrong
/// one is undone from the copy.
pub fn bind_in_file(
    documents: &Path,
    backups: &Path,
    stamp: u64,
    action: &Action,
    pressed: Option<&Pressed>,
) -> Result<PathBuf, String> {
    let at = super::levels::File::Controls.at(documents);
    let before = std::fs::read_to_string(&at)
        .map_err(|_| "the game has not written its controls yet — set them up once in the game")?;
    super::levels::back_up_controls(documents, backups, stamp, 20)?;
    let after = match pressed {
        Some(pressed) => bind(&before, action, pressed),
        None => unbind(&before, action),
    };
    std::fs::write(&at, after).map_err(|e| format!("{}: {e}", at.display()))?;
    Ok(at)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    const FILE: &str = "[HEADER]\nINPUT_METHOD=WHEEL\n\n[CONTROLLERS]\nCON0=Logitech G29\nPGUID0=C24F046D-0000-0000-0000-504944564944\n\n[STEER]\nJOY=0\nAXLE=0 ; the wheel\n\n[GEARUP]\nJOY=0\nBUTTON=4\n\n[GEARDN]\nJOY=0\nBUTTON=5\n";

    fn g29(button: u32) -> Pressed {
        Pressed {
            device: "Logitech G29 Driving Force Racing Wheel".into(),
            ids: Some((0x046d, 0xc24f)),
            button,
        }
    }

    #[test]
    fn the_product_identity_is_directinputs() {
        assert_eq!(
            product_guid(0x046d, 0xc24f),
            "C24F046D-0000-0000-0000-504944564944"
        );
    }

    #[test]
    fn every_section_is_one_the_game_reads_and_none_twice() {
        let mut seen = std::collections::HashSet::new();
        for action in ACTIONS {
            assert!(seen.insert((action.section, action.key)), "{action:?}");
        }
        assert_eq!(groups().first(), Some(&"Gears"));
    }

    /// **A button on a known wheel binds to that wheel's place**, found by
    /// its product even though the system names it differently — and the
    /// axis lines, the comment and the other bindings stay.
    #[test]
    fn a_press_on_a_known_wheel_binds_to_its_place() {
        let drs = ACTIONS.iter().find(|a| a.section == "DRS").unwrap();
        let after = bind(FILE, drs, &g29(7));
        assert_eq!(ini::value(&after, "DRS", "JOY").as_deref(), Some("0"));
        assert_eq!(ini::value(&after, "DRS", "BUTTON").as_deref(), Some("7"));
        assert!(after.contains("AXLE=0 ; the wheel"));
        assert_eq!(controllers(&after).len(), 1);
        let (controller, button) = bound(&after, drs).unwrap();
        assert_eq!((controller.name.as_str(), button), ("Logitech G29", 7));

        // Already on another action: said, not silently taken.
        let up = ACTIONS.iter().find(|a| a.section == "GEARUP").unwrap();
        assert_eq!(taken_by(&after, &g29(4), drs), vec![*up]);
        assert!(taken_by(&after, &g29(9), drs).is_empty());
    }

    /// **A device the file does not know is added at the end**, with its
    /// product, and an H-shifter gear turns the shifter on.
    #[test]
    fn a_new_device_is_added_and_the_shifter_switched_on() {
        let box_ = Pressed {
            device: "Button Box".into(),
            ids: Some((0x1234, 0xabcd)),
            button: 2,
        };
        let third = ACTIONS.iter().find(|a| a.key == "GEAR_3").unwrap();
        let after = bind(FILE, third, &box_);
        let known = controllers(&after);
        assert_eq!(known.len(), 2);
        assert_eq!(known[1].name, "Button Box");
        assert_eq!(
            known[1].product.as_deref(),
            Some("ABCD1234-0000-0000-0000-504944564944")
        );
        assert_eq!(ini::value(&after, "SHIFTER", "JOY").as_deref(), Some("1"));
        assert_eq!(
            ini::value(&after, "SHIFTER", "GEAR_3").as_deref(),
            Some("2")
        );
        assert_eq!(
            ini::value(&after, "SHIFTER", "ACTIVE").as_deref(),
            Some("1")
        );

        // Unbound: nothing reads as bound.
        let up = ACTIONS.iter().find(|a| a.section == "GEARUP").unwrap();
        assert!(bound(&unbind(&after, up), up).is_none());
        assert!(bound(&unbind(&after, third), third).is_none());
    }

    #[test]
    fn a_device_without_ids_is_matched_by_name() {
        let pressed = Pressed {
            device: "logitech g29".into(),
            ids: None,
            button: 1,
        };
        let horn = ACTIONS.iter().find(|a| a.section == "ACTION_HORN").unwrap();
        assert_eq!(controllers(&bind(FILE, horn, &pressed)).len(), 1);
    }

    #[test]
    fn binding_in_the_file_keeps_a_copy_first() {
        let dir = std::env::temp_dir().join(format!("acpe-bind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cfg")).unwrap();
        let drs = ACTIONS.iter().find(|a| a.section == "DRS").unwrap();
        assert!(bind_in_file(&dir, &dir.join("kept"), 1, drs, Some(&g29(3))).is_err());
        std::fs::write(dir.join("cfg/controls.ini"), FILE).unwrap();
        bind_in_file(&dir, &dir.join("kept"), 1, drs, Some(&g29(3))).unwrap();
        let now = std::fs::read_to_string(dir.join("cfg/controls.ini")).unwrap();
        assert_eq!(bound(&now, drs).map(|(_, b)| b), Some(3));
        let copies = super::super::levels::backups(&dir.join("kept"));
        assert_eq!(copies.len(), 1);
        assert_eq!(std::fs::read_to_string(&copies[0]).unwrap(), FILE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **A whole layout kept and put back**: every binding it names in
    /// force, the ones it does not name cleared, the devices found by
    /// product in a file that lists them in another order — and the axes
    /// untouched.
    #[test]
    fn a_layout_is_kept_and_put_back_whatever_the_order_of_devices() {
        let drs = ACTIONS.iter().find(|a| a.section == "DRS").unwrap();
        let horn = ACTIONS.iter().find(|a| a.section == "ACTION_HORN").unwrap();
        let with_box = bind(
            &bind(FILE, drs, &g29(7)),
            horn,
            &Pressed {
                device: "Button Box".into(),
                ids: Some((0x1234, 0xabcd)),
                button: 2,
            },
        );
        let kept = layout(&with_box);
        assert_eq!(kept.len(), 4, "two paddles, DRS and the horn");

        // Another evening: the box enumerated first, DRS moved, a new binding.
        let other = "[CONTROLLERS]\nCON0=Button Box\nPGUID0=ABCD1234-0000-0000-0000-504944564944\n\
                     CON1=Logitech G29\nPGUID1=C24F046D-0000-0000-0000-504944564944\n\
                     [STEER]\nJOY=1\nAXLE=0\n[DRS]\nJOY=1\nBUTTON=1\n[KERS]\nJOY=0\nBUTTON=9\n";
        let back = with_layout(other, &kept);
        let on = |section: &str| {
            let action = ACTIONS.iter().find(|a| a.section == section).unwrap();
            bound(&back, action).map(|(c, b)| (c.name, b))
        };
        assert_eq!(on("DRS"), Some(("Logitech G29".to_string(), 7)));
        assert_eq!(on("ACTION_HORN"), Some(("Button Box".to_string(), 2)));
        assert_eq!(on("GEARUP"), Some(("Logitech G29".to_string(), 4)));
        assert_eq!(on("KERS"), None, "not in the layout: cleared");
        assert_eq!(ini::value(&back, "STEER", "JOY").as_deref(), Some("1"));
        assert_eq!(controllers(&back).len(), 2);
        // It survives being written down.
        let json = serde_json::to_string(&kept).unwrap();
        assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), kept);
    }
}
