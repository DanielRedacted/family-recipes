---
title: Home network
summary: How phones and tablets reach the Mac, the HTTP and HTTPS listeners, the certificate and what to do when a device can't connect.
audience: admin
order: 74
---

The Mac app serves the same UI and API to the desktop window and to every device on the home network. The code is `crates/core/src/lan.rs` (listeners, certificates, Bonjour) and `crates/core/src/auth.rs` (who may do what). For the family's side of it, see [Phones and tablets](#/settings/help/phones-and-tablets).

## The three listeners

| Listener | Address | Port | Used by |
|---|---|---|---|
| `Loopback` | `127.0.0.1` | random, chosen at launch | the desktop window only |
| `LanHttp` | `http://recipes.local` | 80, or 8080 if 80 is taken | any device, no setup |
| `Lan` | `https://recipes.local` | 443, or 8443 if 443 is taken | devices that trust the certificate (needed for the installable offline app) |

All three run the same router (`server::router`). Each request is tagged with the listener it came in on, and that decides how it is authenticated.

- The plain-HTTP listener needs nothing on the device. The PIN, names and session cookie cross the Wi-Fi unencrypted, as with a home file share.
- The HTTPS listener is only needed for the home-screen app (PWA), which keeps working for reading while the Mac is asleep.
- On a device opened over `http://`, the page probes `https://<same host>/api/ping`, then the same on port 8443. If the device already trusts the certificate a probe succeeds and the page moves to that HTTPS address by itself; otherwise it stays on HTTP (`ui/src/main.ts`). Another app holding 443 fails the certificate check, so a Mac that fell back to 8443 is still found.

Sharing is on by default (`settings.lan`) and is switched in Settings → Phones & tablets. Closing the window keeps sharing from the menu-bar item; quitting the app stops it.

## Host names that are accepted

Every request goes through a host check (`AppState::host_allowed`, against DNS rebinding). Anything else gets `421 unknown host`.

- Loopback: `127.0.0.1:<port>` or `localhost:<port>`.
- Home network: `recipes.local` and the Mac's current private IPv4 addresses, on any port.

The Mac's own Bonjour name (for example `Someones-MacBook.local`) is not on the list, so it does not work as an address.

## The certificate

The app is its own certificate authority. Everything lives in the data folder under `tls/`:

| File | What it is |
|---|---|
| `ca.pem`, `ca.key` | the CA, named "Recipes on" plus the Mac's name, valid 10 years |
| `leaf.pem`, `leaf.key`, `leaf.json` | the server certificate for `recipes.local` and the current IPs, valid 397 days |
| `Recipes-CA.crt` | written when the CA is trusted on this Mac |

- The CA carries name constraints: it can only vouch for `recipes.local` and private addresses (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, fc00::/7). A device that trusts it is not exposed to certificates for any other site.
- The leaf is reissued when the Mac's IP addresses change or fewer than 30 days remain. The CA stays the same, so devices don't need to trust it again.
- `GET /ca.crt` downloads the CA in DER form as `Recipes-CA.crt` (404 until sharing has started once).
- The SHA-256 fingerprint is shown in Settings → Phones & tablets → Details and on the `/setup` page, so a device can check it got the genuine certificate.
- **Trust the certificate on this Mac too** (same Details panel) calls `POST /api/setup/trust-local`, which adds the CA to the login keychain with `security add-trusted-cert`; macOS asks for the password. After that `https://recipes.local` works in the Mac's own browsers.

## The device setup page

`/setup` is served only on the plain-HTTP listener. It is the one-time step for the offline app:

1. Download the certificate (`/ca.crt`).
2. Trust it, with steps for iPhone/iPad, Android, another Mac and Windows (the device's own section opens by itself).
3. Open `https://recipes.local` and add it to the home screen.

A device that already trusts the certificate is sent straight to HTTPS. On a phone, the page is reached from Settings → This device → **Install as an app**, shown only while the page is open over `http://`.

## Bonjour name

`recipes.local` is registered with the macOS mDNSResponder by running `/usr/bin/dns-sd -P Recipes _https._tcp local <https port> recipes.local <ip>` (a proxy registration for the Mac's first private IPv4 address). Nothing else is installed.

- A small shell watchdog wraps `dns-sd`: if Recipes dies without cleaning up, the registration goes away within seconds. Its pid is kept in `dns-sd.pid` in the data folder, and a stale one is killed at the next start.
- Every 15 seconds the app checks its addresses. When they change (another Wi-Fi, a new DHCP lease) it reissues the leaf certificate, reloads it without restarting the listener, and registers the new address.
- Only Wi-Fi and Ethernet-like interfaces count; VPN (`utun`), bridges, AirDrop (`awdl`) and similar are skipped.

## One Mac shares at a time

Before it starts sharing, a Mac looks up `recipes.local` (`dns-sd -G v4`, up to 2 seconds). If the name points at an address that isn't its own and a Recipes server answers `GET /api/session` there on port 80, it leaves the network to that Mac. Its own window still shows its own recipes folder, and Settings → Phones & tablets says which Mac is sharing. Once a minute it tries again and takes over when the other Mac has stopped.

## Sign-in and cookies

| Who | How | Cookie |
|---|---|---|
| Desktop window | opens `/auth?k=<launch token>` on the loopback listener; the token is new at every launch | `recipes_admin` (HttpOnly, SameSite=Strict, ends with the session) |
| Device over HTTPS | `POST /api/session` with name and PIN | `__Host-recipes` (Secure, HttpOnly, SameSite=Strict, 400 days) |
| Device over plain HTTP | the same | `recipes_session` (HttpOnly, SameSite=Strict, 400 days), because browsers refuse `__Host-` cookies without HTTPS |

- `/auth` only works on the loopback listener, so the admin cookie can't be obtained from another device.
- A session cookie is `<expiry>.<PIN generation>.<person id>.<signature>`, signed with HMAC-SHA256 using `cookie.key` in the data folder. Either session cookie is accepted on both home-network listeners.
- Changing or removing the PIN bumps the PIN generation and signs every device out. Removing a person in Settings → People signs out their devices.
- Wrong PINs are rate limited: after 5 failures from one address it waits 60 seconds, doubling up to about an hour, and the whole server allows at most 20 failures in 10 minutes (`429 Too Many Requests`).

Roles and the API rules are on [Server API](#/settings/help/server-api); people and the PIN on [People](#/settings/help/people).

## Keeping the Mac awake

**Keep this Mac awake while sharing** runs `caffeinate -i -s -w <app pid>`, which stops idle sleep until it is switched off or the app quits. While the Mac sleeps, the installed app can still show recipes it opened before, but can't see changes or edit.

## When a device can't connect

Work through these on the Mac, in Settings → Phones & tablets:

1. **Is sharing on and running?** The QR code shows only while it runs. A red line shows the error, such as "Not connected to a home network (no private IP address)" or "ports [443, 8443] are all in use".
2. **Is another Mac sharing?** The page says so and gives its address. Devices are talking to that Mac and its recipes folder.
3. **Same network?** The device must be on the network the Mac's addresses (Details → Addresses) belong to.
4. **`recipes.local` not found?** Tap **Device can't find recipes.local? Use the IP address** and scan that QR code instead. If Details → Name doesn't say "advertised", the Bonjour registration failed.
5. **macOS blocked it?** After an update macOS may ask again whether Recipes may accept incoming network connections; choose **Allow**. A denied prompt blocks every device.
6. **The page says "unknown host"?** The address used isn't `recipes.local` or one of the Mac's current IPs (for example an old IP, or the Mac's own `.local` name). Use the address from the QR code.
7. **It opens but can't edit?** No PIN is set, or the device isn't signed in: devices without both are read-only.
8. **HTTPS or the installed app doesn't work?** Check the certificate was trusted in full (on iPhone and iPad that is the extra Certificate Trust Settings switch) and compare the fingerprint. If the Mac fell back to port 8443, the HTTPS address has `:8443` in it.
9. **Nothing changes and it was working?** The Mac may be asleep. Turn on Keep this Mac awake, or open the lid.

The headless `recipes-server` only shares when started with `--lan`; see [Development](#/settings/help/development).
