# Changelog

Every fix or change bumps the last number (2.0.x). Downloads for each version
come from the **Build apps** workflow (Actions tab).

**Not tested yet:** a live Discord session with VC Alert 2, and the Windows
app on a real Windows PC. The engine tests run on macOS and on GitHub's
Windows machine.

## 2.0.7 (2026-10-22)

### Changed
- The status dot sits in the centre of the status pill's rounded end.
- Each person's picture has the same space above, below and to its left
  inside their card, in both list densities.
- The status pill sits a little closer to the buttons, so short channel names
  like "General" still fit at the smallest window size.

### Fixed
- macOS: opening VC Alert from the Dock or menu bar no longer switches you to
  the desktop (Space) it was last on. The window comes to the desktop you're
  on.

## 2.0.6 (2026-10-22)

### Fixed
- Windows: no more command-prompt window opening next to VC Alert. The app is
  now built as a normal Windows program.

## 2.0.5 (2026-10-22)

### Changed
- The watch list has no scrollbar, so the space beside the cards is even.
  A thin blurple bar above "Universal sound" fills from left to right as you
  scroll, and only shows when the list is longer than the window.
- Hovering a person shows only the round ⋯ button. Their own sound is under
  **Sound…** in that menu.

## 2.0.4 (2026-10-22)

### Changed
- The watch list is a set of cards: picture, name, then @username or ID, and
  "joined 2 min ago" while they're in your channel.
- People with their own sound get a **Custom** tag.
- Section headings read "In your channel · 2" and "Everyone else · 5".

## 2.0.3 (2026-10-22)

### Changed
- One-row header: a status pill (state, channel and how many of your list
  are here), round Quiet and Settings buttons, then Start/Stop. Longer
  messages unfold on a line below.
- When someone joins, the pill shows "<name> joined" while the count goes up.

### Fixed
- Profile pictures: sharper (128 px), shown for people who share Discord's
  default picture, and shown in the overlay position editor instead of
  letters.

## 2.0.2 (2026-10-22)

### Fixed
- Opening VC Alert yourself always shows the window. With "Start in the
  background", "Keep running when the window is closed" and "Hide the Dock
  icon" all on, a launch showed nothing at all and looked like a crash.
  "Start in the background" now only applies when VC Alert opens at login.

## 2.0.1 (2026-10-22)

### Fixed
- The app froze with a spinning cursor when pressing Start or Stop.
- After a successful connection test, the button goes back to
  "Start watching" instead of staying on "Fix in Settings".

### Added
- Save up to 3 of your own beeps as profiles (names up to 12 characters).
  They're kept in your settings and in backups.

### Changed
- The default beep is **Classic**: 1700 Hz, 90 ms, 3 beeps, 75% volume.

## 2.0.0 (2026-10-22)

The first release of VC Alert 2, rebuilt as a desktop app for macOS and
Windows.

- A quiet beep and a small always-on-top bar of profile pictures when people
  on your list join the Discord voice channel you're in.
- Talks only to the Discord app on your own computer: no bot, no account
  token, nothing posted anywhere.
- Your own beep or sound file (WAV, MP3, OGG, FLAC) for each person.
- Quiet mode, an overlay you can place anywhere that clicks go through, and
  an option to hide it from screen sharing.
- Share your list with friends as a .json file, or make a full backup.
  Imports show exactly what changes before anything is applied.
