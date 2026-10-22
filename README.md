<p align="center">
  <a href="https://github.com/SURVIVGAMINGPRO/Discord-VC-Alert/releases/latest/download/VC-Alert-macOS.dmg"><b>Download for macOS</b></a>
  &nbsp;·&nbsp;
  <a href="https://github.com/SURVIVGAMINGPRO/Discord-VC-Alert/releases/latest/download/VC-Alert-Windows-setup.exe"><b>Download for Windows</b></a>
</p>

<p align="center"><img src="assets/appicon.svg" width="72" height="72" alt=""></p>

# VC Alert

A quiet beep, and a tiny picture in the corner of your screen, when people you
choose join the Discord voice channel you're in.

It talks only to the Discord app on your own computer. No bot, no account
token, nothing posted anywhere, and nobody else can see it.

What's new in each version: [CHANGELOG.md](CHANGELOG.md).

## Install

Both buttons at the top always download the newest version. Every version is
also on the [Releases page](https://github.com/SURVIVGAMINGPRO/Discord-VC-Alert/releases).

- **macOS:** open `VC-Alert-macOS.dmg` and drag VC Alert to
  Applications. The app isn't signed with an Apple Developer ID yet, so the
  first time macOS blocks it: open **System Settings → Privacy & Security**,
  scroll down and choose **Open Anyway**.
- **Windows:** run `VC-Alert-Windows-setup.exe` (no admin needed), or use the
  portable [`VC-Alert-Windows-portable.exe`](https://github.com/SURVIVGAMINGPRO/Discord-VC-Alert/releases/latest/download/VC-Alert-Windows-portable.exe). Windows may show "Windows protected your PC": choose
  **More info → Run anyway**. Put an empty file named `portable` next to the
  portable .exe to keep its settings beside it.

The Discord **desktop** app must be running. The browser version of Discord
doesn't work.

## First run

VC Alert walks you through this, but here it is in full. It takes about two
minutes, once.

1. Open the [Discord Developer Portal](https://discord.com/developers/applications)
   and choose **New Application**. Any boring name works; nobody else sees it.
2. Copy the **Application ID**.
3. Under **OAuth2**, choose **Reset Secret** and copy the **Client Secret**.
4. Under **OAuth2 → Redirects**, add `http://localhost` and save.
5. If there's an **App Testers** section, add your own account.
6. In VC Alert, paste both into **Settings → Connection** and choose **Save**,
   then **Test connection**. The first time, Discord shows a popup inside the
   Discord app: approve it. VC Alert remembers the approval.

## Using it

- **Add someone:** type their **unique username** (not their display name) in
  the search box and press Enter. A long numeric user ID works too. Paste
  several names separated by commas or spaces to add them all at once.
- **Start watching**, then join a voice channel, in a server or a group call.
  People from your list who are in your call move to the top, and the overlay
  shows them.
- **Lists:** keep up to 5 lists (names up to 12 characters), say "Gaming" and
  "Work". Click the list name next to the logo to switch, create, rename or
  delete one. VC Alert watches the list you're on, and each list has its own
  people and its own sounds for them.
- **Sounds:** the default is *Classic* (1700 Hz, 90 ms, 3 beeps, 75%). Tune a
  beep and choose **Save as profile** to keep up to 3 of your own, with names
  of up to 12 characters.
- **Own sound:** open a person's ⋯ menu (or right-click them, or press Enter
  on them) and choose **Sound…** to give them their own beep or sound file.
  The same menu has Rename, Copy username and Remove. People with their own
  sound show a **Custom** tag (**Custom · Urgent** when it's a ready-made beep
  or one of your profiles). Delete removes the selected person, with Undo.
- **Leave sound:** when someone on your list leaves your call you hear one
  lower, quieter beep. Switch it off for everyone in **Settings → Sound →
  Sound when someone leaves**, or change it for one person in their
  **Sound…** (Off, Soft, their own beep, or a sound file).
- **Snooze:** someone keeps dropping and rejoining? Their ⋯ menu has **Snooze
  for 1 hour** and **Snooze for the rest of this call**: no join or leave
  sound for them until then. Snoozes end when you quit VC Alert.
- **Quiet:** the bell button turns off sound and the overlay; the list still
  shows who joins. The reminder line under the status has × (hide it until
  next time) and **Don't show again** (bring it back in **Settings → General**).
- **Overlay:** **Settings → Overlay** sets the style (pictures, a count, or
  dots), size, and layout. **Position…** lets you drag it anywhere. It never
  takes focus, and clicks go straight through it.
- **Screen sharing:** VC Alert stays out of screen shares and recordings. To
  show it, switch on **Settings → Overlay → Show in screen sharing** (it asks
  once to make sure).
- **Open at login** with **Start in the background** keeps VC Alert in the
  menu bar when you log in. Opening it yourself always shows the window.
- Use headphones: speakers can leak the beep into your microphone.

## Sharing lists with friends

**Settings → Import and export → Export…**
- **List for friends:** names, usernames and per-person beeps (custom sound
  files can be included). Never your Discord app details or settings.
- **Full backup:** everything, for yourself. Discord app details are included
  only if you tick the box, and the file name then ends in `-private`.

To import, drag a `.json` onto the window, use the add menu next to the search
box, or **Settings → Import and export**. You see exactly what's in the file
before anything changes, and a backup is saved first.

Your friends need their own Discord application (steps 1–6 above).

## Troubleshooting

- **"Reconnecting…: open the Discord desktop app"**: start the Discord desktop
  app; VC Alert keeps trying every 5 seconds.
- **"Setup problem: Discord refused the authorization request"**: check the
  Application ID, and that your account is on the app's tester list.
- **"Token exchange failed"**: check the Client Secret, and that
  `http://localhost` is under OAuth2 → Redirects.
- **No popup in Discord**: look inside the Discord app itself; then
  **Settings → Connection → Forget** and try again.
- **A letter instead of a picture**: VC Alert learns someone's picture the
  first time they're in a voice channel with you while it's watching, and keeps
  it for next time. Until then it shows their first letter.
- **The overlay hides behind a game**: switch the game to borderless or
  windowed. Nothing can draw over exclusive full screen.

When you're done: in Discord, **Settings → Authorized Apps**, remove the app,
then delete the application in the Developer Portal.

## Where things are kept

- Settings: `~/Library/Application Support/VC Alert/` (macOS) or
  `%APPDATA%\VC Alert\` (Windows). Profile pictures are saved in its
  `avatars` folder.
- Your Client Secret and Discord approval: your system's keychain
  (Keychain on macOS, Credential Manager on Windows).

## Building from source

Needs Node 20+ and Rust (rustup). Then:

```
npm install
npx tauri dev                                            # run it
cargo test -p vc-core                                    # engine tests
CI=true npx tauri build --target universal-apple-darwin  # macOS app + DMG
```

For Windows, push the repository to GitHub and run the **Build apps** workflow
(Actions tab). It builds the installer and portable .exe on a Windows machine.
See `CLAUDE.md` for the architecture and the rules this project follows.
