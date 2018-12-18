//! Server side

use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use futures::{
    future::{self, Either},
    stream::futures_unordered,
    Future,
    Stream,
};

use super::dns_resolver::set_dns_config;
use config::Config;
use plugin::{launch_plugin, monitor::create_plugin_monitor, PluginMode};
use relay::{boxed_future, tcprelay::server::run as run_tcp, udprelay::server::run as run_udp};

/// Relay server running on server side.
///
/// ```no_run
/// extern crate tokio;
/// extern crate shadowsocks;
///
/// use shadowsocks::{
///     config::{Config, ServerConfig},
///     crypto::CipherType,
///     relay::server::run,
/// };
///
/// use tokio::prelude::*;
///
/// let mut config = Config::new();
/// config.server = vec![ServerConfig::basic(
///     "127.0.0.1:8388".parse().unwrap(),
///     "server-password".to_string(),
///     CipherType::Aes256Cfb,
/// )];
///
/// let fut = run(config);
/// tokio::run(fut.map_err(|err| panic!("Server run failed with error {}", err)));
/// ```
pub fn run(
    mut config: Config,
    signal_monitor: impl Future<Item = (), Error = io::Error> + Send + 'static,
) -> impl Future<Item = (), Error = io::Error> + Send {
    if let Some(c) = config.get_dns_config() {
        set_dns_config(c);
    }

    let mut vf = Vec::new();

    vf.push(boxed_future(signal_monitor));

    if config.enable_udp {
        // Clone config here, because the config for TCP relay will be modified
        // after plugins started
        let udp_config = Arc::new(config.clone());

        // Run UDP relay before starting plugins
        // Because plugins doesn't support UDP relay
        let udp_fut = run_udp(udp_config);
        vf.push(boxed_future(udp_fut));
    }

    let plugins = launch_plugin(&mut config, PluginMode::Server).expect("Failed to launch plugins");

    let abort_signal = Arc::new(AtomicBool::new(false));
    let plugin_monitor = create_plugin_monitor(plugins, abort_signal.clone());

    // Recreate shared config here
    let config = Arc::new(config);

    let tcp_fut = run_tcp(config.clone());
    vf.push(boxed_future(tcp_fut));

    futures_unordered(vf)
        .into_future()
        .select2(plugin_monitor)
        .then(move |res| match res {
            // future other than `plugin_monitor` has resolved
            Ok(Either::A((_, plugin_monitor))) | Err(Either::A((_, plugin_monitor))) => {
                abort_signal.store(true, Ordering::Relaxed);
                boxed_future(plugin_monitor)
            }
            // `plugin_monitor` has resolved
            _ => boxed_future(future::err(io::Error::new(
                io::ErrorKind::Other,
                "Plugin monitor aborted",
            ))),
        })
}
