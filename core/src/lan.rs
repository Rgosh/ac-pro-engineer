//! What the driver has asked the network to do — one set of rules, both front
//! ends.
//!
//! The sockets live in [`crate::broadcast`]; this is the *wish*, and it is
//! deliberately a plain value. A menu that opened a socket where it was
//! clicked would block the thread that draws for as long as a name takes to
//! resolve — seconds, on a machine with no network — so a front end writes
//! down what is wanted and the thread that already runs sixty times a second
//! reconciles the difference. "Apply" then means something exact: a wish and a
//! state, compared once a tick.
//!
//! # Why this is in the core
//!
//! Because it was in one front end, and the other one could not do any of it.
//! The terminal could share a summary if somebody edited `config.json`, and
//! could not watch anybody at all; the window had modes, a list of everybody
//! on the network and every screen fed from a friend's session. Two programs
//! that ship together and cannot see each other is not a feature with a gap in
//! it, it is two features.
//!
//! # The three answers, in the order somebody needs them
//!
//! 1. **Share.** [`LanWish::share_simply`] — a name, and everything else
//!    chosen. There is one right answer for the port and the rate and it is
//!    not worth a question.
//! 2. **Watch.** [`LanWish::watch_simply`] — listen, announce, and pick a
//!    driver out of [`crate::broadcast::discovery`].
//! 3. **Everything else.** The fields below, which the settings screens spell
//!    out for the person who has two networks, a relay, or a reason.

use serde::{Deserialize, Serialize};

/// The port a session travels on, unless somebody says otherwise.
///
/// One number, so "share" and "watch" agree without either being told. It is
/// the one the addresses in older configurations already use.
pub const PORT: u16 = 9001;

/// How much of the network is switched on, and what a front end does about it.
///
/// **A mode rather than two checkboxes**, because the interesting states are
/// not the four combinations: somebody who is only streaming wants their own
/// screens out of the way, and somebody driving wants them. That is the
/// difference between [`Mode::Share`] and [`Mode::OnAir`], and no pair of
/// switches says it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Mode {
    /// Nothing leaves and nothing is listened for.
    #[default]
    Off,
    /// Driving, with every screen as usual, and sending.
    Share,
    /// Sending, with this front end's own screens put away for a compact
    /// panel. For the machine that is only there to feed somebody else — a
    /// spare laptop beside the rig, or the driver's own screen when they want
    /// the pixels back.
    OnAir,
    /// Watching somebody else and sending nothing.
    Watch,
    /// Both at once: sending this session and watching another.
    Both,
}

impl Mode {
    pub const ALL: [(Mode, &'static str, &'static str); 5] = [
        (Mode::Off, "OFF", "nothing leaves this machine"),
        (Mode::Share, "SHARE", "drive as usual, and send it"),
        (
            Mode::OnAir,
            "ON AIR",
            "send it, and put this front end's screens away",
        ),
        (Mode::Watch, "WATCH", "follow somebody else's session"),
        (Mode::Both, "BOTH", "send this one and follow another"),
    ];

    pub fn sends(self) -> bool {
        matches!(self, Mode::Share | Mode::OnAir | Mode::Both)
    }

    pub fn receives(self) -> bool {
        matches!(self, Mode::Watch | Mode::Both)
    }

    /// Whether this front end's own screens are put away for a compact panel.
    pub fn takes_the_screen(self) -> bool {
        self == Mode::OnAir
    }

    pub fn label(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(mode, _, _)| *mode == self)
            .map(|(_, label, _)| *label)
            .unwrap_or("OFF")
    }

    /// What it does, for the line under the switch.
    pub fn describe(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(mode, _, _)| *mode == self)
            .map(|(_, _, what)| *what)
            .unwrap_or("")
    }
}

/// The five blocks a receiver is asked to draw, in the order
/// [`LanWish::blocks`] holds them.
pub const BLOCKS: [&str; 5] = ["TELEMETRY", "ENGINEER", "SESSION", "TIMING", "FUEL"];

/// What the driver wants the network to do.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LanWish {
    pub mode: Mode,
    /// Where to send: `host:port`, resolved rather than parsed, so a name a
    /// home router hands out works.
    pub share_to: String,
    /// The name that travels with it, so a watcher knows whose numbers these
    /// are.
    pub share_as: String,
    /// How many readings a second to send.
    pub share_hz: f32,
    /// Where to listen: `0.0.0.0:9001` for the network, `127.0.0.1:9001` for
    /// this machine alone.
    pub listen_on: String,
    /// Which blocks of the frame the far end should draw.
    ///
    /// The same five the in-game panel obeys, because it is the same frame.
    /// Switching one off does not shrink the datagram — the fields travel
    /// either way — it says what the driver meant it to show.
    pub blocks: [bool; 5],
    /// Only send while the car is actually on track.
    ///
    /// **For the person who leaves this on.** A session spent in the menus
    /// sends a reading thirty times a second saying nothing is happening; a
    /// watcher then cannot tell a driver who is in the pits from one who has
    /// closed the game. Off by default, because the honest default is to send
    /// what is there.
    pub only_on_track: bool,
    /// How long a link may be silent before a screen says so, in seconds.
    pub quiet_after_s: f32,
    /// Announce this copy on the network, so others can find it without
    /// anybody reading an address out loud.
    ///
    /// **A switch of its own.** Somebody on a shared network may want to watch
    /// a friend without telling the room their name. Off does not stop them
    /// *seeing* anybody: listening is free and says nothing.
    pub announce: bool,
}

impl Default for LanWish {
    fn default() -> Self {
        Self {
            mode: Mode::Off,
            share_to: String::new(),
            share_as: String::new(),
            // **Thirty rather than ten.** What travels is the whole reading
            // and the watching machine builds a session out of it — laps,
            // traces, the map's line — so the rate is how finely their picture
            // of the driving is drawn, not how often a number is refreshed. At
            // two kilobytes a reading this is sixty kilobytes a second, which
            // a home network does not notice.
            share_hz: 30.0,
            listen_on: String::new(),
            blocks: [true; 5],
            only_on_track: false,
            quiet_after_s: 3.0,
            announce: true,
        }
    }
}

impl LanWish {
    /// What the stored configuration asks for.
    pub fn from_config(config: &crate::config::AppConfig) -> Self {
        let overlay = &config.overlay;
        let lan = &config.lan;
        let sending = overlay.broadcast_enabled && !overlay.broadcast_to.trim().is_empty();
        let receiving = overlay.receive_enabled && !overlay.receive_from.trim().is_empty();
        Self {
            // The stored mode wins where it agrees with the addresses; where a
            // configuration was written before modes existed, the addresses
            // decide.
            mode: match (lan.mode, sending, receiving) {
                (mode, true, true) if mode.sends() && mode.receives() => mode,
                (mode @ (Mode::Share | Mode::OnAir), true, false) => mode,
                (Mode::Watch, false, true) => Mode::Watch,
                (_, true, true) => Mode::Both,
                (_, true, false) => Mode::Share,
                (_, false, true) => Mode::Watch,
                (_, false, false) => Mode::Off,
            },
            share_to: overlay.broadcast_to.clone(),
            share_as: overlay.broadcast_name.clone(),
            share_hz: lan.share_hz,
            listen_on: overlay.receive_from.clone(),
            blocks: [
                overlay.show_telemetry,
                overlay.show_engineer,
                overlay.show_session,
                overlay.show_timing,
                overlay.show_fuel,
            ],
            only_on_track: lan.only_on_track,
            quiet_after_s: lan.quiet_after_s,
            announce: lan.announce,
        }
    }

    /// Write it back, so it is still true next time the program starts.
    ///
    /// **The address is kept when sharing is switched off.** Clearing it would
    /// do the same job and would make somebody type it again every time.
    pub fn write_into(&self, config: &mut crate::config::AppConfig) {
        let overlay = &mut config.overlay;
        overlay.broadcast_enabled = self.mode.sends();
        overlay.broadcast_to = self.share_to.clone();
        overlay.broadcast_name = self.share_as.clone();
        overlay.receive_enabled = self.mode.receives();
        overlay.receive_from = self.listen_on.clone();
        overlay.show_telemetry = self.blocks[0];
        overlay.show_engineer = self.blocks[1];
        overlay.show_session = self.blocks[2];
        overlay.show_timing = self.blocks[3];
        overlay.show_fuel = self.blocks[4];
        config.lan.mode = self.mode;
        config.lan.share_hz = self.share_hz;
        config.lan.only_on_track = self.only_on_track;
        config.lan.quiet_after_s = self.quiet_after_s;
        config.lan.announce = self.announce;
    }

    /// **Sharing, with one decision asked of the driver: their name.**
    ///
    /// Everything else here has one right answer and asking for it is how a
    /// feature stops being used. The port is [`PORT`]; the rate is the
    /// default; announcing is on, because being findable is the entire reason
    /// a friend does not have to be told an address; and listening is switched
    /// on as well — a driver who shares is exactly the person somebody else
    /// wants to send *back* to, and a peer that announces port zero cannot be
    /// answered.
    ///
    /// `share_to` is deliberately left as it is. Nobody is sent anything until
    /// a driver is chosen from the list or an address is typed, and a wish
    /// that sends nowhere says so through [`Self::complaint`].
    pub fn share_simply(&mut self, as_name: &str) {
        self.mode = if self.mode.receives() {
            Mode::Both
        } else {
            Mode::Share
        };
        self.share_as = as_name.trim().to_string();
        if self.share_hz <= 0.0 {
            self.share_hz = Self::default().share_hz;
        }
        self.announce = true;
        if self.listen_on.trim().is_empty() {
            self.listen_on = format!("0.0.0.0:{PORT}");
        }
    }

    /// **Watching, with nothing asked at all.**
    ///
    /// Listen on every interface at [`PORT`] and announce, so the driver's
    /// copy lists this one and can be pointed at it from either end. Which
    /// driver to watch is not set here: it arrives from
    /// [`crate::broadcast::discovery`], or from an address somebody types.
    pub fn watch_simply(&mut self) {
        self.mode = if self.mode.sends() {
            Mode::Both
        } else {
            Mode::Watch
        };
        if self.listen_on.trim().is_empty() {
            self.listen_on = format!("0.0.0.0:{PORT}");
        }
        self.announce = true;
    }

    /// Aim at somebody found on the network.
    ///
    /// The address is theirs as *this* machine saw it — see
    /// [`crate::broadcast::discovery::Peer::reachable_at`] for why that is not
    /// the address they think they have.
    pub fn send_to_peer(&mut self, peer: &crate::broadcast::discovery::Peer) {
        self.share_to = peer.address();
        if !self.mode.sends() {
            self.mode = if self.mode.receives() {
                Mode::Both
            } else {
                Mode::Share
            };
        }
    }

    /// Stop everything, keeping every address and name where it was.
    pub fn off(&mut self) {
        self.mode = Mode::Off;
    }

    /// The address to send to, or nothing when it is off or unset.
    pub fn sending_to(&self) -> Option<&str> {
        let target = self.share_to.trim();
        (self.mode.sends() && !target.is_empty()).then_some(target)
    }

    /// The address to listen on, or nothing when it is off or unset.
    pub fn listening_on(&self) -> Option<&str> {
        let listen = self.listen_on.trim();
        (self.mode.receives() && !listen.is_empty()).then_some(listen)
    }

    /// The port this copy listens on, or zero when it does not.
    ///
    /// What goes in an announcement: a peer with no port is a peer nobody can
    /// send to, and the list says so rather than offering a link that would
    /// carry nothing.
    pub fn listening_port(&self) -> u16 {
        self.listening_on()
            .and_then(|address| address.rsplit_once(':'))
            .and_then(|(_, port)| port.parse().ok())
            .unwrap_or(0)
    }

    /// What is stopping this doing what it says, if anything.
    ///
    /// A mode that sends with nowhere to send it is the one mistake somebody
    /// makes at the moment it matters, and a switch cannot show it on its own.
    pub fn complaint(&self) -> Option<&'static str> {
        if self.mode.sends() && self.share_to.trim().is_empty() {
            return Some("sending is on and there is no address to send to");
        }
        if self.mode.receives() && self.listen_on.trim().is_empty() {
            return Some("watching is on and there is no address to listen on");
        }
        None
    }
}

/// Split `host:port` into the two boxes a person types into.
///
/// **Two fields, not one.** One box holding `friend-pc:9001` invites every
/// version of the same question — is it a colon, is the port part of the name,
/// what if the name has one in it — and the answer is to stop asking. A string
/// with no colon is all host; the port box is then empty and shows its hint.
pub fn split_address(address: &str) -> (String, String) {
    let address = address.trim();
    match address.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.chars().all(|d| d.is_ascii_digit()) => {
            (host.to_string(), port.to_string())
        }
        _ => (address.to_string(), String::new()),
    }
}

/// Put them back together, or give nothing when either half is missing.
///
/// A host with no port is not an address, and neither is a port with no host —
/// so an incomplete pair produces an empty string, which everything downstream
/// already reads as "not set".
pub fn join_address(host: &str, port: &str) -> String {
    let (host, port) = (host.trim(), port.trim());
    if host.is_empty() || port.is_empty() {
        return String::new();
    }
    format!("{host}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **One click has to leave a working copy behind.** Every field somebody
    /// would otherwise have had to think about is set, and the only thing
    /// missing is the one nobody else can answer: who to send to.
    #[test]
    fn sharing_simply_asks_for_a_name_and_settles_the_rest() {
        let mut wish = LanWish::default();
        wish.share_simply("Kimi");

        assert_eq!(wish.mode, Mode::Share);
        assert_eq!(wish.share_as, "Kimi");
        assert!(wish.announce, "a driver nobody can find is a driver nobody watches");
        assert_eq!(
            wish.listen_on,
            format!("0.0.0.0:{PORT}"),
            "and can be sent to, so the list has a port to show"
        );
        assert!(wish.share_hz > 0.0);
        assert_eq!(
            wish.complaint(),
            Some("sending is on and there is no address to send to"),
            "the one thing left is the one thing a person has to choose"
        );
    }

    /// Watching asks nothing at all, and is complete on its own.
    #[test]
    fn watching_simply_asks_nothing() {
        let mut wish = LanWish::default();
        wish.watch_simply();

        assert_eq!(wish.mode, Mode::Watch);
        assert_eq!(wish.listen_on, format!("0.0.0.0:{PORT}"));
        assert_eq!(wish.listening_port(), PORT);
        assert_eq!(wish.complaint(), None, "nothing else is needed to watch");
    }

    /// Doing both is one switch after the other rather than a third choice to
    /// find: somebody who is watching and then shares is doing both.
    #[test]
    fn sharing_while_watching_is_both() {
        let mut wish = LanWish::default();
        wish.watch_simply();
        wish.share_simply("Kimi");
        assert_eq!(wish.mode, Mode::Both);

        let mut other = LanWish::default();
        other.share_simply("Kimi");
        other.watch_simply();
        assert_eq!(other.mode, Mode::Both);
    }

    /// **Off keeps the address.** The mistake this prevents is somebody
    /// switching sharing off between sessions and typing an address again
    /// every time.
    #[test]
    fn switching_off_keeps_where_it_was_going() {
        let mut wish = LanWish::default();
        wish.share_simply("Kimi");
        wish.share_to = "192.168.1.42:9001".to_string();
        wish.off();

        assert_eq!(wish.mode, Mode::Off);
        assert_eq!(wish.share_to, "192.168.1.42:9001");
        assert_eq!(wish.sending_to(), None, "kept is not the same as used");
        assert_eq!(wish.listening_on(), None);
    }

    #[test]
    fn an_address_splits_and_joins() {
        assert_eq!(
            split_address("friend-pc:9001"),
            ("friend-pc".to_string(), "9001".to_string())
        );
        // A name with no port is all name, and the port box shows its hint.
        assert_eq!(
            split_address("friend-pc"),
            ("friend-pc".to_string(), String::new())
        );
        assert_eq!(join_address("friend-pc", "9001"), "friend-pc:9001");
        assert_eq!(join_address("friend-pc", ""), "");
        assert_eq!(join_address("", "9001"), "");
    }

    /// A wish goes to the configuration and comes back the same, because both
    /// front ends read it from there and a setting that does not survive a
    /// restart is a setting somebody sets twice.
    #[test]
    fn a_wish_survives_being_written_down() {
        let mut wish = LanWish::default();
        wish.share_simply("Kimi");
        wish.share_to = "192.168.1.42:9001".to_string();
        wish.only_on_track = true;
        wish.share_hz = 45.0;

        let mut config = crate::config::AppConfig::default();
        wish.write_into(&mut config);
        assert_eq!(LanWish::from_config(&config), wish);
    }

    /// A configuration written by an older release has no mode in it, and the
    /// addresses have to decide — otherwise somebody who had sharing on finds
    /// it off after an update.
    #[test]
    fn a_configuration_with_no_mode_is_read_from_its_addresses() {
        let mut config = crate::config::AppConfig::default();
        config.overlay.broadcast_to = "192.168.1.42:9001".to_string();
        config.overlay.broadcast_enabled = true;
        config.overlay.receive_from = String::new();

        assert_eq!(LanWish::from_config(&config).mode, Mode::Share);

        config.overlay.receive_from = "0.0.0.0:9001".to_string();
        assert_eq!(LanWish::from_config(&config).mode, Mode::Both);
    }
}
