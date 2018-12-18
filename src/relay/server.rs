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
use plugin::{launch_plugins, monitor::create_plugin_monitor, PluginMode};
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
        // UDP relay doesn't support plugins so is not dependent on them being started first.
        // Give the relay its own copy of the config, because `launch_plugins` below will modify the config.
        vf.push(boxed_future(run_udp(Arc::new(config.clone()))));
    }

    let plugins = launch_plugins(&mut config, PluginMode::Client).expect("Failed to launch plugins");

    match plugins.is_empty() {
        true => {
            vf.push(boxed_future(run_tcp(Arc::new(config))));
            let f = futures_unordered(vf).into_future().then(|res| -> io::Result<()> {
                match res {
                    Ok(..) => Ok(()),
                    Err((err, ..)) => Err(err),
                }
            });
            boxed_future(f)
        }
        false => {
            let abort_signal = Arc::new(AtomicBool::new(false));
            let plugin_monitor = create_plugin_monitor(plugins, abort_signal.clone());
            vf.push(boxed_future(run_tcp(Arc::new(config))));
            let f = futures_unordered(vf)
                .into_future()
                .select2(plugin_monitor)
                .then(move |res| match res {
                    // Future other than `plugin_monitor` has resolved.
                    Ok(Either::A((_, plugin_monitor))) | Err(Either::A((_, plugin_monitor))) => {
                        abort_signal.store(true, Ordering::Relaxed);
                        boxed_future(plugin_monitor)
                    }
                    // Future `plugin_monitor` has resolved.
                    _ => boxed_future(future::err(io::Error::new(
                        io::ErrorKind::Other,
                        "Plugin monitor aborted",
                    ))),
                });
            boxed_future(f)
        }
    }
}
