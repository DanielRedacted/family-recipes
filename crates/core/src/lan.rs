//! Sharing on the home network: the app over plain HTTP on port 80 (no setup on the device),
//! HTTPS at https://recipes.local with a local CA (needed for the offline home-screen app; the
//! one-time setup page is /setup on port 80), and the name advertised through Bonjour.

use crate::server::{AppState, Listener, write_private};
use anyhow::{Context, Result, anyhow};
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use base64::Engine;
use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DnType, ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer,
    KeyPair, KeyUsagePurpose, NameConstraints, SanType,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

pub const HOST: &str = "recipes.local";

#[derive(Serialize, Clone, Default, Debug)]
pub struct LanStatus {
    pub running: bool,
    pub url: String,
    pub setup_url: String,
    pub ips: Vec<String>,
    pub ip_setup_url: String,
    pub https_port: u16,
    pub ca_sha256: String,
    pub mdns: String,
    pub error: Option<String>,
    pub qr_name: String,
    pub qr_ip: String,
    pub keep_awake: bool,
    /// another Mac already shares Recipes on this network (its address): this one leaves it to that Mac
    pub other: Option<String>,
}

#[derive(Default)]
pub struct Lan {
    pub status: RwLock<LanStatus>,
    tls: Mutex<Option<RustlsConfig>>,
    handles: Mutex<Vec<Handle<SocketAddr>>>,
    dnssd: Mutex<Option<Child>>,
    caffeinate: Mutex<Option<Child>>,
    ips: Mutex<Vec<Ipv4Addr>>,
}

impl Drop for Lan {
    fn drop(&mut self) {
        kill_dnssd(&self.dnssd);
        if let Some(mut ch) = self.caffeinate.lock().unwrap().take() {
            let _ = ch.kill();
        }
    }
}

/// Stop the watchdog wrapper and the dns-sd registration it started.
fn kill_dnssd(slot: &Mutex<Option<Child>>) {
    if let Some(mut c) = slot.lock().unwrap().take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    let _ = Command::new("/usr/bin/pkill").args(["-f", "^/usr/bin/dns-sd -P Recipes _https._tcp"]).status();
}

// ---------- certificates ----------

fn private_ranges() -> Vec<GeneralSubtree> {
    let v4 = |a: [u8; 4], p: u8| GeneralSubtree::IpAddress(CidrSubnet::from_v4_prefix(a, p));
    vec![
        GeneralSubtree::DnsName(HOST.into()),
        v4([10, 0, 0, 0], 8),
        v4([172, 16, 0, 0], 12),
        v4([192, 168, 0, 0], 16),
        v4([169, 254, 0, 0], 16),
        GeneralSubtree::IpAddress(CidrSubnet::from_addr_prefix("fc00::".parse().unwrap(), 7)),
    ]
}

/// The CA can only vouch for recipes.local and private addresses (name constraints), so a
/// device that trusts it is not exposed to certificates for any other site.
fn load_or_create_ca(dir: &Path) -> Result<(String, String)> {
    let (cp, kp) = (dir.join("ca.pem"), dir.join("ca.key"));
    if let (Ok(c), Ok(k)) = (std::fs::read_to_string(&cp), std::fs::read_to_string(&kp)) {
        return Ok((c, k));
    }
    let mut p = CertificateParams::new(Vec::<String>::new())?;
    let host = hostname();
    p.distinguished_name.push(DnType::CommonName, format!("Recipes on {host}"));
    p.distinguished_name.push(DnType::OrganizationName, "Recipes app (local)");
    p.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign, KeyUsagePurpose::DigitalSignature];
    p.name_constraints = Some(NameConstraints { permitted_subtrees: private_ranges(), excluded_subtrees: vec![] });
    let now = time::OffsetDateTime::now_utc();
    p.not_before = now - time::Duration::days(1);
    p.not_after = now + time::Duration::days(3650);
    let key = KeyPair::generate()?;
    let cert = p.self_signed(&key)?;
    write_private(&kp, key.serialize_pem().as_bytes())?;
    write_private(&cp, cert.pem().as_bytes())?;
    Ok((cert.pem(), key.serialize_pem()))
}

#[derive(Serialize, Deserialize, PartialEq)]
struct LeafMeta {
    ips: Vec<Ipv4Addr>,
    not_after: i64,
}

/// Leaf cert for recipes.local + current LAN IPs; reissued when IPs change or <30 days remain.
fn leaf(dir: &Path, ca_pem: &str, ca_key: &str, ips: &[Ipv4Addr]) -> Result<(Vec<u8>, Vec<u8>)> {
    let (lp, lk, lm) = (dir.join("leaf.pem"), dir.join("leaf.key"), dir.join("leaf.json"));
    let now = time::OffsetDateTime::now_utc();
    if let (Ok(c), Ok(k), Ok(m)) = (std::fs::read(&lp), std::fs::read(&lk), std::fs::read(&lm)) {
        if let Ok(meta) = serde_json::from_slice::<LeafMeta>(&m) {
            if meta.ips == ips && meta.not_after - now.unix_timestamp() > 30 * 86400 {
                return Ok((c, k));
            }
        }
    }
    let mut p = CertificateParams::new(vec![HOST.to_string()])?;
    for ip in ips {
        p.subject_alt_names.push(SanType::IpAddress(IpAddr::V4(*ip)));
    }
    p.distinguished_name.push(DnType::CommonName, HOST);
    p.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    p.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    p.not_before = now - time::Duration::days(1);
    p.not_after = now + time::Duration::days(397);
    let key = KeyPair::generate()?;
    let issuer = Issuer::from_ca_cert_pem(ca_pem, KeyPair::from_pem(ca_key)?)?;
    let cert = p.signed_by(&key, &issuer)?;
    let chain = format!("{}{}", cert.pem(), ca_pem);
    write_private(&lp, chain.as_bytes())?;
    write_private(&lk, key.serialize_pem().as_bytes())?;
    let meta = LeafMeta { ips: ips.to_vec(), not_after: p.not_after.unix_timestamp() };
    write_private(&lm, &serde_json::to_vec(&meta)?)?;
    Ok((chain.into_bytes(), key.serialize_pem().into_bytes()))
}

pub fn pem_to_der(pem: &str) -> Vec<u8> {
    let b64: String = pem.lines().skip_while(|l| !l.starts_with("-----BEGIN")).skip(1).take_while(|l| !l.starts_with("-----END")).collect();
    base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default()
}

fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

fn hostname() -> String {
    Command::new("/usr/sbin/scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "this Mac".into())
}

/// Private IPv4 addresses on physical-ish interfaces (Wi-Fi / Ethernet), primary first.
pub fn lan_ips() -> Vec<Ipv4Addr> {
    let mut out: Vec<(String, Ipv4Addr)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !["lo", "utun", "bridge", "awdl", "llw", "vmnet", "anpi", "gif", "stf"].iter().any(|p| i.name.starts_with(p)))
        .filter_map(|i| match i.ip() {
            IpAddr::V4(v4) if v4.is_private() => Some((i.name, v4)),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.1 == b.1);
    out.into_iter().map(|(_, ip)| ip).collect()
}

fn qr(url: &str) -> String {
    qrcode::QrCode::new(url.as_bytes())
        .map(|c| c.render::<qrcode::render::svg::Color>().min_dimensions(180, 180).quiet_zone(true).build())
        .unwrap_or_default()
}

// ---------- servers ----------

/// Start (or restart) LAN sharing according to settings.
pub async fn start(state: Arc<AppState>, app: Router) {
    stop(&state);
    if !state.settings.read().unwrap().lan {
        return;
    }
    if let Err(e) = start_inner(state.clone(), app).await {
        tracing::warn!("network sharing failed: {e:#}");
        let mut s = state.ext.lan.status.write().unwrap();
        s.running = false;
        s.error = Some(format!("{e:#}"));
    }
}

pub fn stop(state: &AppState) {
    let lan = &state.ext.lan;
    for h in lan.handles.lock().unwrap().drain(..) {
        h.graceful_shutdown(Some(Duration::from_secs(2)));
    }
    kill_dnssd(&lan.dnssd);
    set_keep_awake(state, false);
    let mut s = lan.status.write().unwrap();
    s.running = false;
    s.other = None;
}

async fn start_inner(state: Arc<AppState>, app: Router) -> Result<()> {
    let lan = &state.ext.lan;
    let ips = lan_ips();
    // one Mac owns recipes.local: when another already serves it, this one only shows the window
    reap_stale(&state.data_dir);
    if let Some(ip) = other_server(&ips).await {
        tracing::info!("Recipes on {ip} already shares on this network; not sharing from this Mac");
        *lan.status.write().unwrap() = LanStatus { other: Some(ip.to_string()), ..Default::default() };
        return Ok(());
    }
    let dir = state.data_dir.join("tls");
    let (ca_pem, ca_key) = load_or_create_ca(&dir)?;
    let (chain, key) = leaf(&dir, &ca_pem, &ca_key, &ips)?;
    let tls = RustlsConfig::from_pem(chain, key).await.context("loading certificate")?;
    *lan.tls.lock().unwrap() = Some(tls.clone());
    *lan.ips.lock().unwrap() = ips.clone();
    set_hosts(&state, &ips);

    let https_port = bind_first(&[443, 8443])?;
    let http_port = bind_first(&[80, 8080])?;
    let h1 = Handle::new();
    let h2 = Handle::new();
    lan.handles.lock().unwrap().extend([h1.clone(), h2.clone()]);
    let http_app = setup_router(state.clone()).merge(app.clone().layer(Extension(Listener::LanHttp)));
    let https_app = app.layer(Extension(Listener::Lan));
    let svc = https_app.into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move {
        if let Err(e) = axum_server::bind_rustls(SocketAddr::from(([0, 0, 0, 0], https_port)), tls).handle(h1).serve(svc).await {
            tracing::warn!("https listener stopped: {e}");
        }
    });
    tokio::spawn(async move {
        let svc = http_app.into_make_service_with_connect_info::<SocketAddr>();
        if let Err(e) = axum_server::bind(SocketAddr::from(([0, 0, 0, 0], http_port))).handle(h2).serve(svc).await {
            tracing::warn!("http listener stopped: {e}");
        }
    });

    let mdns = advertise(&state, &ips, https_port);
    let port_sfx = |p: u16, def: u16| if p == def { String::new() } else { format!(":{p}") };
    let ca_der = pem_to_der(&ca_pem);
    let primary = ips.first().map(|i| i.to_string()).unwrap_or_default();
    let setup_url = format!("http://{HOST}{}/", port_sfx(http_port, 80));
    let ip_setup_url = if primary.is_empty() { String::new() } else { format!("http://{primary}{}/", port_sfx(http_port, 80)) };
    {
        let mut s = lan.status.write().unwrap();
        *s = LanStatus {
            running: true,
            url: format!("https://{HOST}{}/", port_sfx(https_port, 443)),
            qr_name: qr(&setup_url),
            qr_ip: if ip_setup_url.is_empty() { String::new() } else { qr(&ip_setup_url) },
            setup_url,
            ip_setup_url,
            ips: ips.iter().map(|i| i.to_string()).collect(),
            https_port,
            ca_sha256: fingerprint(&ca_der),
            mdns,
            error: if ips.is_empty() { Some("Not connected to a home network (no private IP address).".into()) } else { None },
            keep_awake: false,
            other: None,
        };
    }
    if state.settings.read().unwrap().keep_awake {
        set_keep_awake(&state, true);
    }
    Ok(())
}

/// Another Mac serving Recipes here: recipes.local resolves to an address that isn't this Mac's and
/// a Recipes server answers there (a stale registration of a Mac that's gone doesn't count).
async fn other_server(own: &[Ipv4Addr]) -> Option<Ipv4Addr> {
    let ip = tokio::task::spawn_blocking(resolve_host).await.ok().flatten()?;
    if own.contains(&ip) {
        return None;
    }
    // ponytail: asks port 80 only, not the 8080 fallback a Mac uses when 80 is taken
    let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build().ok()?;
    let r = client.get(format!("http://{ip}/api/session")).send().await.ok()?;
    r.status().is_success().then_some(ip)
}

/// The IPv4 address recipes.local is registered at on this network (`dns-sd -G`, up to 2 s).
fn resolve_host() -> Option<Ipv4Addr> {
    use std::io::BufRead;
    let mut child = Command::new("/usr/bin/dns-sd")
        .args(["-G", "v4", HOST])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        // "14:05:08.823  Add  40000002  11  recipes.local.  192.168.1.206  240"
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            let ip = line.split_whitespace().nth(5).and_then(|w| w.parse::<Ipv4Addr>().ok());
            if line.contains(" Add ") && ip.is_some_and(|i| !i.is_unspecified()) {
                let _ = tx.send(ip.unwrap());
                return;
            }
        }
    });
    let ip = rx.recv_timeout(Duration::from_secs(2)).ok();
    let _ = child.kill();
    let _ = child.wait();
    ip
}

fn bind_first(ports: &[u16]) -> Result<u16> {
    for p in ports {
        if std::net::TcpListener::bind(("0.0.0.0", *p)).is_ok() {
            return Ok(*p);
        }
    }
    Err(anyhow!("ports {ports:?} are all in use"))
}

fn set_hosts(state: &AppState, ips: &[Ipv4Addr]) {
    let mut hosts = vec![HOST.to_string()];
    hosts.extend(ips.iter().map(|i| i.to_string()));
    *state.lan_hosts.write().unwrap() = hosts;
}

/// Register recipes.local -> IP with the system mDNSResponder (`dns-sd -P` proxy registration).
fn advertise(state: &AppState, ips: &[Ipv4Addr], port: u16) -> String {
    let lan = &state.ext.lan;
    kill_dnssd(&lan.dnssd);
    let Some(ip) = ips.first() else { return "waiting for a network".into() };
    reap_stale(&state.data_dir);
    // Watchdog wrapper: if Recipes dies without cleaning up (crash, force quit), the
    // registration goes away within seconds instead of advertising a dead address.
    let script = format!(
        "/usr/bin/dns-sd -P Recipes _https._tcp local {port} {HOST} {ip} >/dev/null 2>&1 & D=$!; \
         while kill -0 {me} 2>/dev/null && kill -0 $D 2>/dev/null; do sleep 2; done; kill $D 2>/dev/null",
        me = std::process::id()
    );
    match Command::new("/bin/sh")
        .args(["-c", &script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => {
            let _ = std::fs::write(state.data_dir.join("dns-sd.pid"), child.id().to_string());
            *lan.dnssd.lock().unwrap() = Some(child);
            "advertised".into()
        }
        Err(e) => format!("could not advertise recipes.local: {e}"),
    }
}

/// Kill a dns-sd left behind by a crashed previous run.
fn reap_stale(dir: &Path) {
    let Ok(pid) = std::fs::read_to_string(dir.join("dns-sd.pid")) else { return };
    let pid = pid.trim();
    let cmd = Command::new("/bin/ps").args(["-p", pid, "-o", "command="]).output();
    if cmd.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("dns-sd -P Recipes")) {
        let _ = Command::new("/bin/kill").arg(pid).status();
    }
    let _ = Command::new("/usr/bin/pkill").args(["-f", "^/usr/bin/dns-sd -P Recipes _https._tcp"]).status();
    let _ = std::fs::remove_file(dir.join("dns-sd.pid"));
}

/// Watch for IP changes (Wi-Fi switch, DHCP renewal): reissue the leaf cert and re-advertise.
pub fn spawn_ip_watch(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = 0u32;
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let (running, other) = {
                let s = state.ext.lan.status.read().unwrap();
                (s.running, s.other.is_some())
            };
            if !running {
                // left to another Mac: take over once a minute when it has stopped sharing
                tick += 1;
                if other && tick % 4 == 0 && state.settings.read().unwrap().lan {
                    if let Some(app) = state.ext.app.get().cloned() {
                        start(state.clone(), app).await;
                    }
                }
                continue;
            }
            let ips = lan_ips();
            if *state.ext.lan.ips.lock().unwrap() == ips {
                continue;
            }
            tracing::info!("network addresses changed: {ips:?}");
            if let Err(e) = refresh(&state, &ips).await {
                tracing::warn!("could not refresh certificate: {e:#}");
            }
        }
    });
}

async fn refresh(state: &Arc<AppState>, ips: &[Ipv4Addr]) -> Result<()> {
    let dir = state.data_dir.join("tls");
    let (ca_pem, ca_key) = load_or_create_ca(&dir)?;
    let (chain, key) = leaf(&dir, &ca_pem, &ca_key, ips)?;
    let tls = state.ext.lan.tls.lock().unwrap().clone();
    if let Some(tls) = tls {
        tls.reload_from_pem(chain, key).await?;
    }
    *state.ext.lan.ips.lock().unwrap() = ips.to_vec();
    set_hosts(state, ips);
    let port = state.ext.lan.status.read().unwrap().https_port;
    let mdns = advertise(state, ips, port);
    let mut s = state.ext.lan.status.write().unwrap();
    s.ips = ips.iter().map(|i| i.to_string()).collect();
    s.mdns = mdns;
    if let Some(ip) = ips.first() {
        let port = s.setup_url.rsplit_once(':').and_then(|(_, p)| p.trim_end_matches('/').parse::<u16>().ok());
        s.ip_setup_url = format!("http://{ip}{}/", port.map(|p| format!(":{p}")).unwrap_or_default());
        s.qr_ip = qr(&s.ip_setup_url);
    }
    Ok(())
}

/// `caffeinate -i -s -w <pid>`: keep the Mac from idle-sleeping while sharing.
pub fn set_keep_awake(state: &AppState, on: bool) {
    let mut c = state.ext.lan.caffeinate.lock().unwrap();
    if let Some(mut ch) = c.take() {
        let _ = ch.kill();
        let _ = ch.wait();
    }
    if on {
        *c = Command::new("/usr/bin/caffeinate").args(["-i", "-s", "-w", &std::process::id().to_string()]).spawn().ok();
    }
    state.ext.lan.status.write().unwrap().keep_awake = c.is_some();
}

/// Add the CA to the login keychain as trusted (macOS asks for the user's password).
pub fn trust_on_this_mac(state: &AppState) -> Result<()> {
    let dir = state.data_dir.join("tls");
    let (ca_pem, _) = load_or_create_ca(&dir)?;
    let crt = dir.join("Recipes-CA.crt");
    std::fs::write(&crt, pem_to_der(&ca_pem))?;
    let home = std::env::var("HOME")?;
    let st = Command::new("/usr/bin/security")
        .args(["add-trusted-cert", "-r", "trustRoot", "-k", &format!("{home}/Library/Keychains/login.keychain-db")])
        .arg(&crt)
        .status()?;
    if !st.success() {
        return Err(anyhow!("macOS did not add the certificate (cancelled?)"));
    }
    Ok(())
}

// ---------- device setup page (port 80, merged in front of the app) ----------

fn setup_router(state: Arc<AppState>) -> Router {
    Router::new().route("/setup", get(setup_page)).with_state(state) // /ca.crt is an app route
}

pub async fn ca_cert(State(s): State<Arc<AppState>>) -> Response {
    let dir = s.data_dir.join("tls");
    match std::fs::read_to_string(dir.join("ca.pem")) {
        Ok(pem) => (
            [
                (header::CONTENT_TYPE, "application/x-x509-ca-cert"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"Recipes-CA.crt\""),
            ],
            pem_to_der(&pem),
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "network sharing has not been set up").into_response(),
    }
}

async fn setup_page(State(s): State<Arc<AppState>>) -> Response {
    let st = s.ext.lan.status.read().unwrap().clone();
    let html = include_str!("setup.html")
        .replace("{{FINGERPRINT}}", &st.ca_sha256)
        .replace("{{HTTPS_PORT}}", &if st.https_port == 443 || st.https_port == 0 { String::new() } else { format!(":{}", st.https_port) });
    ([(header::CACHE_CONTROL, "no-store")], Html(html)).into_response()
}
