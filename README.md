Inhibits idle on [Wayland](https://en.wikipedia.org/wiki/Wayland_(display_server_protocol)) during media playback by an [MPRIS2](https://specifications.freedesktop.org/mpris-spec/latest/) player

## Installation

Archlinux users can install [`aur/sway-mpris-idle-inhibit`](https://aur.archlinux.org/packages/sway-mpris-idle-inhibit)

## Usage

In Sway config (~/.config/sway/config):

```
# Inhibit idle during playback
exec wl-mpris-idle-inhibit
```

## TODOs

- [ ] User specified player ignore list (i.e. music applications)
- [ ] Investigate feasibility of distinguishing video playback
