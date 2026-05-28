# CLAUDE.md — zerokb

## Open incident — WiFi not associating (2026-05-28)

**Symptom:** odin on agneta logs `zerokb: send failed (No route to host)` then `(timed out)` on press 2. Pi Zero 2W at 192.168.10.8 unreachable from the LAN — no ping, no ARP response, no mDNS. Pi has green-LED power and the HID gadget enumerates fine on the host (mbair sees `Keychron V3`, `bcdDevice=0x0102`, matching `scripts/hid-setup.sh`). So rootfs boots and `zerokb-hid.service` starts; only WiFi is dead.

**What we found and partially fixed today:**

- `ansible/deploy-zerokb.yml:126` pins WiFi to a single BSSID (`46:E9:31:4A:2E:5C`) via `nmcli con modify preconfigured 802-11-wireless.bssid`. The SSID `deco.synology.austrheim` is a multi-AP mesh, so a hard BSSID lock kills roaming and breaks the Pi if that one AP is out of range from its physical spot (next to the host laptop).
- That BSSID was visible from agneta at 92% signal, so the AP itself is alive — but agneta and the Pi aren't co-located, so visibility there ≠ visibility at the Pi.
- Pulled the SD card into ingalisa (`/dev/sdc`, `sdc2` = ext4 rootfs). Removed only the `bssid=...` line from `/etc/NetworkManager/system-connections/preconfigured.nmconnection`. Backup kept alongside as `preconfigured.nmconnection.bak.bssid-removed` (NM ignores non-`.nmconnection` files). SSID + PSK + IP config untouched.
- Put the card back. Pi booted (HID gadget enumerates on mbair) but still does **not** associate WiFi. So BSSID lock was *a* problem but not the *only* problem.

**Resume here tomorrow:**

1. **Try a long power-drain first** — unplug *all* USB from the Pi, count 20s, replug. Pi Zero 2W's BCM43436 firmware sometimes wedges across short reboots; full drain clears it. Then `ssh -o ConnectTimeout=4 agneta 'ping -c2 192.168.10.8'`.
2. If still dead, pull the SD card back into ingalisa and read `/var/log/syslog` (or `journalctl --directory=/mnt/zerokb-root/var/log/journal` if persistent) for the last boot's NetworkManager / wpa_supplicant errors. Note that the deploy sets journald to volatile per the README, so journal may be empty after reboot — `/var/log/syslog` is the fallback.
3. Sanity-check the WiFi PSK in `preconfigured.nmconnection` against whatever `deco.synology.austrheim` currently uses (in case the mesh password rotated).
4. Verify radio is even up on boot: look for `brcmfmac` firmware-load errors in dmesg.

**Playbook follow-ups (do these once the Pi is back up, not before):**

- Drop the BSSID lock in `ansible/deploy-zerokb.yml:125-129`. Roaming-within-SSID is what we actually want on a Deco mesh. The `wifi.powersave=2` line above it already pins the radio awake, which was the real anti-drop measure.
- Add a USB ECM gadget alongside HID in `scripts/hid-setup.sh` so the Pi is reachable as a USB network device from the host even when WiFi is dead. Today's incident had no remote recovery path — only physical SD-card pull. That should never be the only option.
- Optional: ship the Pi onto tailscale so it has a second independent network path (mesh-independent and routable from any agent).

**Artefacts on the SD card (still there):**

- `/etc/NetworkManager/system-connections/preconfigured.nmconnection` — edited (no `bssid=` line)
- `/etc/NetworkManager/system-connections/preconfigured.nmconnection.bak.bssid-removed` — pre-edit copy, safe to delete after the dust settles

---

## What this is

zerokb is the USB-HID typer for the [loki](https://github.com/3loc/loki) meeting-intelligence rig. Runs on a Raspberry Pi Zero 2W. Receives bytes on TCP `:7070`, types them on the host as a USB keyboard (identifies as Keychron V3). See `README.md` for the deploy story.

## Layout

```
zerokb/
├── ansible/
│   ├── deploy-zerokb.yml   # first-time deploy: HID gadget, NM config, watchdog
│   ├── inventory.yml       # zero@192.168.10.8
│   └── upgrade.yml         # cargo-zigbuild + scp + systemctl restart
├── scripts/
│   └── hid-setup.sh        # libcomposite gadget for Keychron V3 (0x3434:0x0333)
├── systemd/
│   ├── zerokb.service      # Rust binary listening on :7070
│   └── zerokb-hid.service  # runs hid-setup.sh on boot
└── src/                    # Rust source — TCP byte → USB HID scancode
```

## Key config files (on the Pi)

- `/etc/NetworkManager/system-connections/preconfigured.nmconnection` — WiFi (SSID `deco.synology.austrheim`)
- `/sys/kernel/config/usb_gadget/zerokb/` — runtime libcomposite gadget tree
- `/etc/systemd/system/zerokb.service` + `/etc/systemd/system/zerokb-hid.service`

## Talking to the Pi

```
ssh zero@192.168.10.8                          # SSH (WiFi)
echo "hi" | nc 192.168.10.8 7070                # send bytes to type
agneta:~$ systemctl --user status odin          # the orchestrator that sends to us
```
