//! Who the driver is on Steam, and who their friends are — read from the
//! Steam client's own files, with no account to make and nothing asked of
//! any server.
//!
//! **No registration, because Steam already did it.** The signed-in account
//! is in `loginusers.vdf` ([`crate::steam::signed_in`]). The client keeps a
//! copy of each friend's name in the user's `localconfig.vdf`, under
//! `userdata/<account id>/config/`, and the avatars it has drawn in
//! `config/avatarcache/<id64>.png`. All three are on disk on every machine
//! Steam has run on, so a profile can be drawn offline, at once, and without
//! one byte about anybody leaving the computer.
//!
//! **What this cannot do, and why it says so.** A SteamID is public, which
//! is exactly why it proves nothing: anybody can type anybody's. A profile
//! that *other people* see needs the owner to prove the id is theirs —
//! Steam's own sign-in — and a server to keep it; that is not this. These
//! are the driver's own facts, on the driver's own machine.

use std::path::PathBuf;

/// The offset between a 64-bit SteamID of an individual account and its
/// 32-bit account id — the number `userdata/` folders are named by.
pub const ID64_BASE: u64 = 76_561_197_960_265_728;

/// The account id inside a 64-bit SteamID, when it is one of a person.
pub fn account_id(id64: &str) -> Option<u32> {
    let id: u64 = id64.trim().parse().ok()?;
    let account = id.checked_sub(ID64_BASE)?;
    u32::try_from(account).ok()
}

/// The 64-bit SteamID of an account id.
pub fn id64(account_id: u32) -> String {
    (ID64_BASE + u64::from(account_id)).to_string()
}

/// The public page of an account.
pub fn profile_url(id64: &str) -> String {
    format!("https://steamcommunity.com/profiles/{}", id64.trim())
}

/// A value in Valve's key-value format: text, or a block of more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vdf {
    Text(String),
    Block(Vec<(String, Vdf)>),
}

impl Vdf {
    /// The value under `key`, case as Steam writes it ignored.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Block(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Vdf::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Vdf::Text(text) => Some(text),
            Vdf::Block(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Block(entries) => entries,
            Vdf::Text(_) => &[],
        }
    }
}

/// The deepest a file may nest: Steam's go five or six deep, and a file
/// that goes deeper is not one of Steam's.
const DEEPEST: usize = 32;

/// Read Valve's key-value text. Whatever does not parse ends the block it is
/// in; nothing in it can make this panic or recurse without end.
pub fn parse(text: &str) -> Vdf {
    let tokens = tokens(text);
    let mut at = 0;
    Vdf::Block(block(&tokens, &mut at, 0))
}

#[derive(Debug, PartialEq)]
enum Token {
    Word(String),
    Open,
    Close,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut word = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => word.push('\n'),
                            Some('t') => word.push('\t'),
                            Some(other) => word.push(other),
                            None => break,
                        },
                        c => word.push(c),
                    }
                }
                out.push(Token::Word(word));
            }
            c if c.is_whitespace() => {}
            // A bare word, as some files write their keys.
            c => {
                let mut word = c.to_string();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                out.push(Token::Word(word));
            }
        }
    }
    out
}

fn block(tokens: &[Token], at: &mut usize, depth: usize) -> Vec<(String, Vdf)> {
    let mut entries = Vec::new();
    while let Some(token) = tokens.get(*at) {
        *at += 1;
        let key = match token {
            Token::Word(key) => key.clone(),
            Token::Close => return entries,
            Token::Open => continue,
        };
        match tokens.get(*at) {
            Some(Token::Word(value)) => {
                *at += 1;
                entries.push((key, Vdf::Text(value.clone())));
            }
            Some(Token::Open) => {
                *at += 1;
                if depth >= DEEPEST {
                    // Skip to the matching close without reading what is in it.
                    let mut open = 1usize;
                    while let Some(token) = tokens.get(*at) {
                        *at += 1;
                        match token {
                            Token::Open => open += 1,
                            Token::Close => {
                                open -= 1;
                                if open == 0 {
                                    break;
                                }
                            }
                            Token::Word(_) => {}
                        }
                    }
                    continue;
                }
                entries.push((key, Vdf::Block(block(tokens, at, depth + 1))));
            }
            Some(Token::Close) | None => return entries,
        }
    }
    entries
}

/// One of the driver's Steam friends, as the client last saw them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Friend {
    pub id64: String,
    /// The name they go by now.
    pub name: String,
    /// Names they went by before, newest first — what they may still race
    /// under on a server.
    pub before: Vec<String>,
}

/// The friends in a `localconfig.vdf`.
pub fn friends_in(localconfig: &str) -> Vec<Friend> {
    let root = parse(localconfig);
    let store = root.get("UserLocalConfigStore").unwrap_or(&root);
    let Some(friends) = store.get("friends") else {
        return Vec::new();
    };
    let mut out: Vec<Friend> = friends
        .entries()
        .iter()
        .filter_map(|(key, value)| {
            // Each friend is a block keyed by their account id; the rest of
            // the section is the driver's own settings.
            let account: u32 = key.parse().ok()?;
            let name = value.get("name")?.text()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let before = value
                .get("NameHistory")
                .map(|history| {
                    history
                        .entries()
                        .iter()
                        .filter_map(|(_, v)| v.text())
                        .map(|n| n.trim().to_string())
                        .filter(|n| !n.is_empty() && *n != name)
                        .collect()
                })
                .unwrap_or_default();
            Some(Friend {
                id64: id64(account),
                name,
                before,
            })
        })
        .collect();
    out.sort_by_key(|f| f.name.to_lowercase());
    out.dedup_by(|a, b| a.id64 == b.id64);
    out
}

/// The `localconfig.vdf` of an account, in the first Steam root that has it.
pub fn localconfig(roots: &[PathBuf], id64: &str) -> Option<PathBuf> {
    let account = account_id(id64)?;
    roots
        .iter()
        .map(|root| {
            root.join("userdata")
                .join(account.to_string())
                .join("config")
                .join("localconfig.vdf")
        })
        .find(|at| at.is_file())
}

/// The driver's friends, from the Steam client's own copy.
pub fn friends(roots: &[PathBuf], id64: &str) -> Vec<Friend> {
    localconfig(roots, id64)
        .and_then(|at| std::fs::read_to_string(at).ok())
        .map(|text| friends_in(&text))
        .unwrap_or_default()
}

/// The avatar the Steam client last drew for an account, when it kept one.
pub fn avatar(roots: &[PathBuf], id64: &str) -> Option<PathBuf> {
    account_id(id64)?;
    roots
        .iter()
        .map(|root| {
            root.join("config")
                .join("avatarcache")
                .join(format!("{}.png", id64.trim()))
        })
        .find(|at| at.is_file())
}

/// Whether a name a driver races under is one of this friend's.
pub fn is_named(friend: &Friend, name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && std::iter::once(&friend.name)
            .chain(friend.before.iter())
            .any(|n| n.trim().eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    const LOCALCONFIG: &str = r#""UserLocalConfigStore"
{
	"friends"
	{
		"PersonaName"		"Rgosh"
		"communitypreferences"		"0a02..."
		"12345678"
		{
			"name"		"Kimi \"Iceman\""
			"tag"		""
			"avatar"		"fef49e7fa7e1997310d705b2a6158ff8dc1cdfeb"
			"NameHistory"
			{
				"0"		"Kimi \"Iceman\""
				"1"		"K. Raikkonen"
			}
		}
		"87654321"
		{
			"name"		"anna"
		}
		"99"
		{
			"tag"		"no name"
		}
	}
	"Software" { "Valve" { "Steam" { "x" "y" } } }
}
"#;

    #[test]
    fn an_id64_and_its_account_id_are_the_same_person() {
        assert_eq!(account_id("76561197972531406"), Some(12_265_678));
        assert_eq!(id64(12_265_678), "76561197972531406");
        assert_eq!(account_id("12"), None);
        assert_eq!(account_id("not a number"), None);
        assert_eq!(account_id(" 76561197960265728 "), Some(0));
        assert_eq!(
            profile_url("76561197972531406"),
            "https://steamcommunity.com/profiles/76561197972531406"
        );
    }

    /// **Friends read from the client's own file**: names with quotes in
    /// them, old names kept for matching a server's list, the driver's own
    /// settings and a friend with no name left out, sorted by name.
    #[test]
    fn friends_are_read_from_the_clients_own_file() {
        let friends = friends_in(LOCALCONFIG);
        assert_eq!(friends.len(), 2);
        assert_eq!(friends[0].name, "anna");
        assert_eq!(friends[1].name, "Kimi \"Iceman\"");
        assert_eq!(friends[1].id64, id64(12_345_678));
        assert_eq!(friends[1].before, vec!["K. Raikkonen"]);
        assert!(is_named(&friends[1], "k. raikkonen"));
        assert!(!is_named(&friends[1], ""));
        assert!(!is_named(&friends[0], "Kimi"));
    }

    #[test]
    fn the_files_are_found_where_steam_keeps_them() {
        let root = std::env::temp_dir().join(format!("acpe-steamprof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let me = id64(4242);
        std::fs::create_dir_all(root.join("userdata/4242/config")).unwrap();
        std::fs::create_dir_all(root.join("config/avatarcache")).unwrap();
        std::fs::write(
            root.join("userdata/4242/config/localconfig.vdf"),
            LOCALCONFIG,
        )
        .unwrap();
        std::fs::write(root.join(format!("config/avatarcache/{me}.png")), b"png").unwrap();
        let roots = vec![root.join("missing"), root.clone()];
        assert_eq!(friends(&roots, &me).len(), 2);
        assert!(avatar(&roots, &me).is_some());
        assert!(friends(&roots, &id64(1)).is_empty());
        assert!(avatar(&roots, "../../etc/passwd").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **Nothing in a file stops it**: unbalanced braces, an unclosed
    /// quote, a file nested far past anything Steam writes, and random
    /// bytes all read as something or nothing, never a panic.
    #[test]
    fn nothing_in_a_file_stops_it() {
        for text in [
            "",
            "{",
            "}",
            "\"a\"",
            "\"a\" {",
            "\"friends\" { \"1\" { \"name\" \"x",
            "\"UserLocalConfigStore\" { \"friends\" { \"5\" { \"name\" \"ok\" } } } } } }",
        ] {
            let _ = friends_in(text);
        }
        let deep = "\"a\" {".repeat(10_000) + &"}".repeat(10_000);
        let _ = parse(&deep);
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..300 {
            let bytes: Vec<u8> = (0..200)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    b"{}\" \\\n/abc0123"[(seed % 14) as usize]
                })
                .collect();
            let _ = friends_in(&String::from_utf8_lossy(&bytes));
        }
        assert_eq!(
            friends_in("\"UserLocalConfigStore\" { \"friends\" { \"5\" { \"name\" \"ok\" } } } }")
                .len(),
            1
        );
    }
}
