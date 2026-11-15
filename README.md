# Meeting Display

A Windows tray app that keeps your next calendar meeting on a [BUSY Bar](https://busy.app)
plugged in over USB, and walks you through each meeting: approach, countdown, start,
time left, overrun.

- **User guide, build and release** (in French): [`app/README.md`](app/README.md)
- **Wheel and buttons on the bar itself**: the optional on-bar companion app, [`bar-app/README.md`](bar-app/README.md)
- **What is tested and what is not**: [`docs/STATUS.md`](docs/STATUS.md)
- **Download**: the latest installer is on the [Releases](../../releases/latest) page.
  Installed copies update themselves.

It reads a published ICS calendar link (Outlook, or any other), needs no account and talks
to the bar only through its local HTTP API.

## Credits

The calendar icon shown on the bar (`app/src-tauri/assets/calendar.png`) comes from the
official [Clock app](https://github.com/busy-app/clock-app) by Flipper Devices, MIT licence.
BUSY Bar is a product of Flipper Devices; this project is not affiliated with them.
