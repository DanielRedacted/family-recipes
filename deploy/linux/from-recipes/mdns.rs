use if_addrs::get_if_addrs;
use mdns_sd::{ServiceDaemon, ServiceInfo};

/// Registers `recipes.local` on the LAN via mDNS/Bonjour.
///
/// The returned `ServiceDaemon` must be kept alive — dropping it
/// unregisters the service. Failures are logged as warnings so a
/// missing mDNS stack never prevents the app from starting.
pub fn register(port: u16) -> Option<ServiceDaemon> {
    // Pick the first non-loopback IPv4 address on this host.
    let ip = match get_if_addrs() {
        Ok(ifaces) => ifaces
            .into_iter()
            .find(|i| !i.is_loopback() && i.ip().is_ipv4())
            .map(|i| i.ip()),
        Err(e) => {
            tracing::warn!("mDNS: could not enumerate network interfaces: {e}");
            return None;
        }
    };

    let Some(ip) = ip else {
        tracing::warn!("mDNS: no non-loopback IPv4 interface found — skipping registration");
        return None;
    };

    let mdns = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("mDNS: daemon failed to start: {e}");
            return None;
        }
    };

    let info = match ServiceInfo::new(
        "_http._tcp.local.",
        "Recipe App",
        "recipes.local.",
        ip.to_string().as_str(),
        port,
        None,
    ) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("mDNS: could not build service info: {e}");
            return None;
        }
    };

    match mdns.register(info) {
        Ok(_) => tracing::info!("mDNS: recipes.local:{port} → {ip}"),
        Err(e) => {
            tracing::warn!("mDNS: registration failed: {e}");
            return None;
        }
    }

    Some(mdns)
}
