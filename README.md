Inhibits idle on [Wayland](https://en.wikipedia.org/wiki/Wayland_(display_server_protocol)) during media playback by an [MPRIS2](https://specifications.freedesktop.org/mpris-spec/latest/) player

## Installation

Archlinux users can install [`aur/sway-mpris-idle-inhibit`](https://aur.archlinux.org/packages/sway-mpris-idle-inhibit)

## Usage

In Sway config (~/.config/sway/config):

```
# Inhibit idle during playback
exec sway-mpris-idle-inhibit
```
