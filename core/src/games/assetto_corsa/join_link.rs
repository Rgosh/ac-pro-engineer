//! Links that ask a launcher to do something: join a server, install a mod.
//!
//! **How people are invited to a race.** A league's page, a Discord
//! announcement and every hosting panel hand out one of two things: a web
//! address at `acstuff.club/s/q:race/online/join?ip=…&httpPort=…` (older
//! ones at `acstuff.ru`), whose page has a button, and the address that
//! button opens — `acmanager://race/online/join?ip=…&httpPort=…`. Both name
//! the server by its address and its **HTTP** port, never the port the game
//! connects to: whoever opens the link asks the server for the rest.
//!
//! The scheme is Content Manager's. Reading it is what lets a link written
//! for that program open in any other; nothing here is that program's code,
//! only the shape of an address people already publish.
//!
//! **Read leniently, made strictly.** A link arrives pasted from a chat with
//! a trailing full stop, with `&amp;` where a forum escaped it, with the
//! scheme in capitals. What this *writes* is the one form every reader takes.

/// What a link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Join the server at this address.
    Join {
        /// An IP address or a host name, as the link gives it.
        host: String,
        /// The server's HTTP port — the one that answers `/INFO`.
        http: u16,
        /// The password, when the link carries one.
        password: Option<String>,
    },
    /// Download what is at this address and install it.
    Install { url: String },
}

/// Read a link, or `None` when the text is not one of these.
pub fn parse(text: &str) -> Option<Link> {
    // What a chat or a forum wraps round an address.
    let text = text
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '<' | '"' | '\'' | '('))
        .trim_end_matches(|c: char| {
            c.is_whitespace() || matches!(c, '>' | '"' | '\'' | '.' | ',' | ')')
        })
        .replace("&amp;", "&");
    let lower = text.to_ascii_lowercase();
    let (path, query) = match text.split_once('?') {
        Some((path, query)) => (path.to_ascii_lowercase(), query),
        None => (lower.clone(), ""),
    };
    let ours = lower.starts_with("acmanager:")
        || lower.contains("acstuff.club/s/")
        || lower.contains("acstuff.ru/s/");
    if !ours {
        return None;
    }
    let field = |name: &str| {
        query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| unescaped(value))
            .filter(|value| !value.is_empty())
    };
    if path.contains("race/online") {
        let host = field("ip")?;
        // A host is letters, digits, dots, dashes and — for IPv6 — colons
        // and brackets. Anything else is not an address, and it is about to
        // be put in a URL.
        if !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
        {
            return None;
        }
        let http = field("httpPort")
            .or_else(|| field("port"))?
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)?;
        return Some(Link::Join {
            host,
            http,
            password: field("password").or_else(|| field("plainPassword")),
        });
    }
    if path.contains("install") {
        let url = field("url")?;
        let secure = url.to_ascii_lowercase().starts_with("https://")
            || url.to_ascii_lowercase().starts_with("http://");
        return secure.then_some(Link::Install { url });
    }
    None
}

/// The web address that invites somebody to a server — the form a browser
/// opens, with a page and a button, so it works pasted anywhere.
pub fn invite(host: &str, http: u16) -> String {
    format!("https://acstuff.club/s/q:race/online/join?ip={host}&httpPort={http}")
}

/// The same invitation as the address a launcher is opened with directly.
pub fn direct(host: &str, http: u16) -> String {
    format!("acmanager://race/online/join?ip={host}&httpPort={http}")
}

/// `%41` as `A` and `+` as a space, the way a query string is written.
/// Bytes that do not come out as text are dropped rather than guessed at.
fn unescaped(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'%' => {
                let hex = bytes
                    .get(at + 1..at + 3)
                    .and_then(|pair| std::str::from_utf8(pair).ok())
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok());
                match hex {
                    Some(decoded) => {
                        out.push(decoded);
                        at += 3;
                    }
                    None => {
                        out.push(byte);
                        at += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                at += 1;
            }
            other => {
                out.push(other);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(host: &str, http: u16, password: Option<&str>) -> Option<Link> {
        Some(Link::Join {
            host: host.to_string(),
            http,
            password: password.map(str::to_string),
        })
    }

    /// The two forms people are handed, and what a chat does to them.
    #[test]
    fn an_invitation_reads_in_every_form_it_is_handed_out_in() {
        assert_eq!(
            parse("acmanager://race/online/join?ip=203.0.113.7&httpPort=8081"),
            join("203.0.113.7", 8081, None)
        );
        assert_eq!(
            parse("https://acstuff.club/s/q:race/online/join?ip=203.0.113.7&httpPort=8081"),
            join("203.0.113.7", 8081, None)
        );
        assert_eq!(
            parse("http://acstuff.ru/s/q:race/online/join?ip=203.0.113.7&httpPort=9616"),
            join("203.0.113.7", 9616, None)
        );
        // Pasted from a chat: angle brackets, a full stop, an escaped
        // ampersand, capitals in the scheme and the keys.
        assert_eq!(
            parse("  <ACManager://race/online/join?IP=race.example.org&amp;HTTPPORT=8081>. "),
            join("race.example.org", 8081, None)
        );
    }

    #[test]
    fn a_password_in_the_link_is_carried_and_unescaped() {
        assert_eq!(
            parse("acmanager://race/online/join?ip=10.0.0.5&httpPort=8081&password=friday%20night"),
            join("10.0.0.5", 8081, Some("friday night"))
        );
    }

    /// What is not an invitation is not guessed into one.
    #[test]
    fn anything_else_is_not_a_link() {
        for not in [
            "",
            "203.0.113.7:9600",
            "https://example.org/race/online/join?ip=1.2.3.4&httpPort=8081",
            "acmanager://race/online/join?ip=1.2.3.4",
            "acmanager://race/online/join?httpPort=8081",
            "acmanager://race/online/join?ip=1.2.3.4&httpPort=0",
            "acmanager://race/online/join?ip=1.2.3.4&httpPort=port",
            "acmanager://race/online/join?ip=1.2.3.4/../x&httpPort=8081",
            "acmanager://something/else?ip=1.2.3.4&httpPort=8081",
        ] {
            assert_eq!(parse(not), None, "{not}");
        }
    }

    #[test]
    fn an_install_link_names_what_to_download() {
        assert_eq!(
            parse("acmanager://install?url=https%3A%2F%2Fexample.org%2Fcar.zip"),
            Some(Link::Install {
                url: "https://example.org/car.zip".to_string()
            })
        );
        // Only something a download can be started from.
        assert_eq!(parse("acmanager://install?url=file%3A%2F%2F%2Fetc"), None);
    }

    /// What this writes, it reads — both forms, for any address.
    #[test]
    fn a_link_made_here_reads_back() {
        for (host, http) in [("203.0.113.7", 8081), ("race.example.org", 9616)] {
            assert_eq!(parse(&invite(host, http)), join(host, http, None));
            assert_eq!(parse(&direct(host, http)), join(host, http, None));
        }
    }
}
