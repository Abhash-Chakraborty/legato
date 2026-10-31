//! stand-in-phone: pair with Legato and sync from the command line.
//!
//!   stand-in-phone pair '<pairing link from Settings › Devices › Copy link>'
//!   stand-in-phone connect
//!
//! Then type commands (or pipe them in), one per line:
//!   word <text>            add a Dictionary word      unword <text>   remove it
//!   snippet <trigger> = <text>                        dictate <text>  a phone dictation
//!   scratch <title> = <text>                          clip <text>     copy to the shared clipboard
//!   list <kind>            dictionary, snippets, history, notes, scratchpad, transforms, style
//!   sync                   send changes now and ask for the PC's
//!   wait <seconds>         unpair                     quit
//!   help                   list these
//!
//! Options: --dir <folder> (default ./phone), --name <name> (default "Stand-in phone").

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use legato_phone::sync::identity::uuid;
use legato_phone::sync::link::{Command, Sessions};
use legato_phone::sync::protocol::{Clip, ClipKind, PairLink};
use legato_phone::{LinkEnd, PairedPc, Phone};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut take = |flag: &str| -> Option<String> {
        let at = args.iter().position(|a| a == flag)?;
        let value = args.get(at + 1).cloned();
        args.drain(at..(at + 2).min(args.len()));
        value
    };
    let dir = PathBuf::from(take("--dir").unwrap_or_else(|| "phone".into()));
    let name = take("--name").unwrap_or_else(|| "Stand-in phone".into());
    // Check the subcommand before opening the phone, so a bare run for the
    // usage text leaves no folder behind.
    if !matches!(args.first().map(String::as_str), Some("pair" | "connect")) {
        eprintln!("usage: stand-in-phone [--dir <folder>] [--name <name>] pair '<link>' | connect");
        std::process::exit(2);
    }
    let phone = Arc::new(Phone::open(
        &dir,
        &name,
        Box::new(|line| println!("· {line}")),
    )?);

    let mut secret = None;
    if args.first().map(String::as_str) == Some("pair") {
        let Some(uri) = args.get(1) else {
            bail!("pair needs the pairing link");
        };
        let link = PairLink::parse(uri)?;
        phone.save_pc(&PairedPc {
            device_id: link.device_id,
            name: link.name,
            fingerprint: link.fingerprint,
            addrs: link.addrs,
        })?;
        secret = Some(link.secret);
    }
    let Some(pc) = phone.paired_pc() else {
        bail!("not paired; run `stand-in-phone pair '<link>'` first");
    };
    phone.refresh()?;

    let sessions = Sessions::default();
    let (stop_tx, stop) = watch::channel(false);
    let link_task = {
        let (phone, sessions) = (phone.clone(), sessions.clone());
        tokio::spawn(async move {
            if let LinkEnd::Refused(why) = phone.link_loop(sessions, secret, stop).await {
                println!("· {why}");
                std::process::exit(1);
            }
        })
    };

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let pair = || rest.split_once(" = ").map(|(a, b)| (a.trim(), b.trim()));
        let edited = match command {
            "" => false,
            "word" => phone.add_word(rest).map(|_| true)?,
            "unword" => phone.remove_word(rest).map(|_| true)?,
            "dictate" => phone.dictate(rest, "").map(|_| true)?,
            "snippet" | "scratch" => {
                let Some((a, b)) = pair() else {
                    println!("· {command} needs `<name> = <text>`");
                    continue;
                };
                if command == "snippet" {
                    phone.add_snippet(a, b)?;
                } else {
                    phone.scratch(a, b)?;
                }
                true
            }
            "clip" => {
                let clip = Clip {
                    id: uuid(),
                    kind: ClipKind::Text,
                    text: Some(rest.to_string()),
                    png: None,
                    age_ms: 0,
                    from_app: Some("Stand-in".into()),
                };
                if !sessions.send(&pc.device_id, Command::Clip(clip)) {
                    println!("· Not connected; the clip wasn't sent");
                }
                false
            }
            "list" => {
                for item in phone.list(rest)? {
                    println!("  {item}");
                }
                false
            }
            "sync" => {
                phone.refresh()?;
                sessions.send(&pc.device_id, Command::Nudge);
                sessions.send(&pc.device_id, Command::Pull);
                false
            }
            "wait" => {
                let secs: f64 = rest.parse().unwrap_or(1.0);
                tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                false
            }
            "unpair" => {
                sessions.send(&pc.device_id, Command::Bye("unpaired".into()));
                tokio::time::sleep(Duration::from_millis(300)).await;
                phone.forget_pc()?;
                println!("· Unpaired");
                break;
            }
            "quit" | "exit" => break,
            "help" => {
                println!("{HELP}");
                false
            }
            other => {
                println!("· Unknown command: {other}; type `help`");
                false
            }
        };
        if edited {
            let changed = phone.refresh()?;
            if changed > 0 {
                sessions.send(&pc.device_id, Command::Nudge);
            }
        }
    }
    let _ = stop_tx.send(true);
    let _ = tokio::time::timeout(Duration::from_secs(2), link_task).await;
    Ok(())
}

const HELP: &str = "\
  word <text>                 add a Dictionary word
  unword <text>               remove it
  snippet <trigger> = <text>  add a snippet
  dictate <text>              a phone dictation, into history
  scratch <title> = <text>    write a Scratchpad note
  clip <text>                 copy to the shared clipboard
  list <kind>                 dictionary, snippets, history, notes, scratchpad, transforms, style
  sync                        send changes now and ask for the PC's
  wait <seconds>              pause, for piped scripts
  unpair                      forget the PC and tell it
  quit                        disconnect";
