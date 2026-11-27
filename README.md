# Syncie

*Tools you don't hear, but feel.* — a small, free Windows tool by [Liofree](https://liofree.com).

Syncie backs up your folders to **Google Drive** or a **local / USB drive** and then gets out of your way.
The interface is currently in German.

## What it does

- **Mirror with a recycle bin** – the backup follows your folders. Files you delete locally move to a
  recycle bin in the backup and stay there for 1–12 months. Syncie warns you a few days before
  anything is removed for good.
- **One-way per folder** – mark a folder as *Einweg* and files deleted locally stay in the backup forever.
- **Schedule** – pick days and a time; Syncie registers a task in the Windows Task Scheduler.
- **History and notifications** – every run is logged (files, locked files, errors, duration).
  Optional Windows notification after each run or only when something went wrong.
- **Robust** – parallel uploads, resumes after a crash, retries on Google rate limits,
  skips files that are locked by another program and tells you which ones.
- **Nothing leaves your PC** except your files, and only to the target you choose. No telemetry.

## Download

Get the latest `syncie-gui.exe` and `syncie-cli.exe` from the
[Releases](https://github.com/liofree/syncie/releases) page. Keep both files in the same folder –
the scheduled task runs `syncie-cli.exe`.

## Google Drive

Sign in with Google inside Syncie (*Profil → Wohin → Google Drive*). For now this needs your own
OAuth client ID from the Google Cloud Console (Desktop app, Drive API enabled); the steps are shown
in the app. A one-click sign-in is planned.

## Build from source

Requires Rust (stable) on Windows.

```
cargo build --release
cargo test --release
```

The binaries end up in `target/release/`.

## Command line

```
syncie-cli --list                     list profiles
syncie-cli --run [--profile NAME]     back up all profiles or one
syncie-cli --due                      only profiles scheduled for today
```

Output is one JSON line; exit code 0 = ok, 1 = error.

## Privacy

See [PRIVACY.md](PRIVACY.md).

## Support

Syncie is free. If it saves you time, you can [buy me a coffee on Ko-fi](https://ko-fi.com/liofree).

## License

[MIT](LICENSE) © 2026 Liofree
