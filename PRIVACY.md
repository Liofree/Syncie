# Syncie – Privacy Policy

*Last updated: 27 November 2026*

Syncie is a desktop backup tool published by Liofree (https://liofree.com). It runs entirely on your
computer. Liofree does not operate any server for Syncie and does not receive any of your data.

## What Syncie accesses

- **Your local folders** – only the folders you add to a profile, to copy them to your backup target.
- **Google Drive (optional)** – only if you choose Google Drive as a target. Syncie uploads your files
  to the Drive folder you select, lists and reads the files it created there (to detect changes and to
  restore files from its recycle bin) and deletes backup copies according to your retention setting.

## What Syncie stores

All settings stay on your computer in `%APPDATA%\syncie`: profiles, run history, backup state and the
Google sign-in token. Nothing is sent to Liofree or any third party. There is no telemetry, no
analytics and no advertising.

## Google user data

Syncie's use and transfer of information received from Google APIs adheres to the
[Google API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy),
including the Limited Use requirements. Data from Google Drive is used only to provide the backup
feature you set up, is never transferred to anyone else and is never used for advertising or to
train AI models.

You can revoke Syncie's access at any time at https://myaccount.google.com/permissions and delete
the local token by removing `%APPDATA%\syncie\drive_token.json`.

## Contact

Questions: open an issue at https://github.com/liofree/syncie or use the contact options on
https://liofree.com.
