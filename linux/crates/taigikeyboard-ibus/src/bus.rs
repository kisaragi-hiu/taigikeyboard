//! Finding and joining the daemon's private bus (roadmap L1), as
//! `ibus_get_address` does (ibus `src/ibusshare.c:130-285`): `$IBUS_ADDRESS`
//! when set, else the `IBUS_ADDRESS=` line of the socket file
//! `$XDG_CONFIG_HOME/ibus/bus/<machine-id>-<hostname>-<display>`, and only
//! while the `IBUS_DAEMON_PID=` it names is alive.
//!
//! The daemon's bus is a message bus (it answers `Hello` and `RequestName`
//! on `org.freedesktop.DBus` itself, `src/ibusbus.c:508-555`, `:1283`), so
//! `zbus::connection::Builder::address` — which performs the Hello — is the
//! whole connection.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum BusAddressError {
    #[error("no display: neither WAYLAND_DISPLAY nor DISPLAY is set")]
    NoDisplay,
    #[error("no XDG_CONFIG_HOME and no HOME to find the ibus socket file under")]
    NoConfigHome,
    #[error("could not read the ibus socket file {path}: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the ibus socket file {path} names no IBUS_ADDRESS")]
    NoAddress { path: PathBuf },
    #[error("the ibus daemon (pid {pid}) named by {path} is not running")]
    DaemonGone { path: PathBuf, pid: u32 },
}

/// The bus address to connect to.
pub fn address() -> Result<String, BusAddressError> {
    address_with(
        |name| std::env::var_os(name),
        |path| std::fs::read_to_string(path),
        process_is_alive,
    )
}

pub fn address_with(
    env: impl Fn(&str) -> Option<OsString>,
    read: impl Fn(&Path) -> std::io::Result<String>,
    is_alive: impl Fn(u32) -> bool,
) -> Result<String, BusAddressError> {
    if let Some(address) = env("IBUS_ADDRESS").filter(|value| !value.is_empty()) {
        return Ok(address.to_string_lossy().into_owned());
    }
    let path = socket_path(&env, &read)?;
    let contents = read(&path).map_err(|source| BusAddressError::Unreadable {
        path: path.clone(),
        source,
    })?;
    let parsed = parse_socket_file(&contents);
    let address = parsed
        .address
        .ok_or_else(|| BusAddressError::NoAddress { path: path.clone() })?;
    match parsed.daemon_pid {
        Some(pid) if is_alive(pid) => Ok(address),
        Some(pid) => Err(BusAddressError::DaemonGone { path, pid }),
        // ibus itself refuses a file with no pid; a file the daemon wrote
        // always has one.
        None => Err(BusAddressError::DaemonGone { path, pid: 0 }),
    }
}

/// `$IBUS_ADDRESS_FILE`, else `<config>/ibus/bus/<machine-id>-<host>-<display>`
/// (`ibus_get_socket_path`).
fn socket_path(
    env: &impl Fn(&str) -> Option<OsString>,
    read: &impl Fn(&Path) -> std::io::Result<String>,
) -> Result<PathBuf, BusAddressError> {
    if let Some(path) = env("IBUS_ADDRESS_FILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let (hostname, display_number) = display_parts(env)?;
    let config_home = match env("XDG_CONFIG_HOME") {
        Some(value) if !value.is_empty() && Path::new(&value).is_absolute() => PathBuf::from(value),
        _ => {
            let home = env("HOME").filter(|value| !value.is_empty());
            PathBuf::from(home.ok_or(BusAddressError::NoConfigHome)?).join(".config")
        }
    };
    let machine_id = machine_id(read);
    Ok(config_home
        .join("ibus")
        .join("bus")
        .join(format!("{machine_id}-{hostname}-{display_number}")))
}

/// Wayland: `unix` + the whole `$WAYLAND_DISPLAY`. X11: `$DISPLAY` split as
/// `[host]:number[.screen]`, an empty host reading `unix` (`ibusshare.c:148-184`).
fn display_parts(
    env: &impl Fn(&str) -> Option<OsString>,
) -> Result<(String, String), BusAddressError> {
    if let Some(wayland) = env("WAYLAND_DISPLAY").filter(|value| !value.is_empty()) {
        return Ok(("unix".to_owned(), wayland.to_string_lossy().into_owned()));
    }
    let display = env("DISPLAY")
        .filter(|value| !value.is_empty())
        .ok_or(BusAddressError::NoDisplay)?
        .to_string_lossy()
        .into_owned();
    let (host, rest) = display.split_once(':').unwrap_or((display.as_str(), ""));
    let number = rest.split('.').next().unwrap_or("");
    let hostname = if host.is_empty() { "unix" } else { host };
    let display_number = if number.is_empty() { "0" } else { number };
    Ok((hostname.to_owned(), display_number.to_owned()))
}

/// `/var/lib/dbus/machine-id`, else `/etc/machine-id`, else the literal
/// `machine-id` ibus falls back to (`ibus_get_local_machine_id`).
fn machine_id(read: &impl Fn(&Path) -> std::io::Result<String>) -> String {
    ["/var/lib/dbus/machine-id", "/etc/machine-id"]
        .iter()
        .filter_map(|path| read(Path::new(path)).ok())
        .map(|contents| contents.trim().to_owned())
        .find(|id| !id.is_empty())
        .unwrap_or_else(|| "machine-id".to_owned())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SocketFile {
    address: Option<String>,
    daemon_pid: Option<u32>,
}

fn parse_socket_file(contents: &str) -> SocketFile {
    let mut parsed = SocketFile::default();
    for line in contents.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(address) = line.strip_prefix("IBUS_ADDRESS=") {
            parsed.address = Some(address.trim_end().to_owned());
        } else if let Some(pid) = line.strip_prefix("IBUS_DAEMON_PID=") {
            parsed.daemon_pid = pid.trim().parse().ok();
        }
    }
    parsed
}

/// `kill(pid, 0)` without libc: on Linux a live process has a `/proc/<pid>`
/// directory. (On the macOS host this always answers false, which only the
/// tests exercise through their own closure.)
fn process_is_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), OsString::from(value)))
            .collect();
        move |name| map.get(name).cloned()
    }

    fn files(pairs: &[(&str, &str)]) -> impl Fn(&Path) -> std::io::Result<String> {
        let map: HashMap<PathBuf, String> = pairs
            .iter()
            .map(|(path, contents)| (PathBuf::from(path), (*contents).to_owned()))
            .collect();
        move |path| {
            map.get(path)
                .cloned()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
    }

    const SOCKET_FILE: &str = "# This file is created by ibus-daemon, please do not modify it.\n\
        # This file allows processes on the machine to find the\n\
        # ibus session bus with the below address.\n\
        # If the IBUS_ADDRESS environment variable is set, it will\n\
        # be used rather than this file.\n\
        IBUS_ADDRESS=unix:abstract=/home/u/.cache/ibus/dbus-abc,guid=1234\n\
        IBUS_DAEMON_PID=4242\n";

    #[test]
    fn the_environment_variable_wins_without_touching_the_file() {
        let address = address_with(
            env(&[("IBUS_ADDRESS", "unix:path=/tmp/x")]),
            |_| panic!("no file read"),
            |_| panic!("no pid check"),
        )
        .unwrap();
        assert_eq!(address, "unix:path=/tmp/x");
    }

    #[test]
    fn the_wayland_socket_file_is_machine_unix_display_under_config_home() {
        // trace: `ibus_get_socket_path` — Wayland: hostname "unix",
        // displaynumber = $WAYLAND_DISPLAY; machine id from /etc/machine-id.
        let address = address_with(
            env(&[
                ("HOME", "/home/u"),
                ("XDG_CONFIG_HOME", "/home/u/.config"),
                ("WAYLAND_DISPLAY", "wayland-0"),
                ("DISPLAY", ":0"),
            ]),
            files(&[
                ("/etc/machine-id", "abcdef\n"),
                (
                    "/home/u/.config/ibus/bus/abcdef-unix-wayland-0",
                    SOCKET_FILE,
                ),
            ]),
            |pid| pid == 4242,
        )
        .unwrap();
        assert_eq!(
            address,
            "unix:abstract=/home/u/.cache/ibus/dbus-abc,guid=1234"
        );
    }

    #[test]
    fn the_x11_display_splits_at_colon_and_drops_the_screen() {
        // trace: ":1.0" → hostname "unix", number "1"; "host:2" → "host", "2".
        let files = files(&[
            ("/var/lib/dbus/machine-id", "m1\n"),
            ("/home/u/.config/ibus/bus/m1-unix-1", SOCKET_FILE),
            ("/home/u/.config/ibus/bus/m1-host-2", SOCKET_FILE),
        ]);
        assert!(address_with(
            env(&[("HOME", "/home/u"), ("DISPLAY", ":1.0")]),
            &files,
            |_| true
        )
        .is_ok());
        assert!(address_with(
            env(&[("HOME", "/home/u"), ("DISPLAY", "host:2")]),
            &files,
            |_| true
        )
        .is_ok());
    }

    #[test]
    fn a_dead_daemon_or_a_missing_file_is_an_error_not_a_stale_address() {
        let files = files(&[
            ("/etc/machine-id", "m\n"),
            ("/home/u/.config/ibus/bus/m-unix-0", SOCKET_FILE),
        ]);
        let dead = address_with(
            env(&[("HOME", "/home/u"), ("DISPLAY", ":0")]),
            &files,
            |_| false,
        );
        assert!(matches!(
            dead,
            Err(BusAddressError::DaemonGone { pid: 4242, .. })
        ));
        let missing = address_with(
            env(&[("HOME", "/home/u"), ("DISPLAY", ":9")]),
            &files,
            |_| true,
        );
        assert!(matches!(missing, Err(BusAddressError::Unreadable { .. })));
        assert!(matches!(
            address_with(env(&[("HOME", "/home/u")]), &files, |_| true),
            Err(BusAddressError::NoDisplay)
        ));
    }

    #[test]
    fn comments_are_skipped_and_the_pid_is_parsed() {
        assert_eq!(
            parse_socket_file(SOCKET_FILE),
            SocketFile {
                address: Some("unix:abstract=/home/u/.cache/ibus/dbus-abc,guid=1234".to_owned()),
                daemon_pid: Some(4242),
            }
        );
    }
}
