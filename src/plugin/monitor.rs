use super::Plugin;
use futures::{self, stream::futures_unordered, Future, Stream};
use relay::boxed_future;
use std::{io, thread, time::Duration};

pub fn create_plugin_monitor(plugins: Vec<Plugin>) -> impl Future<Item = (), Error = io::Error> + Send {
    let mut plugin_monitors = Vec::new();
    let mut plugin_aborts = Vec::new();

    for mut plugin in plugins {
        let (tx_monitor, rx_monitor) = futures::sync::oneshot::channel();
        let (tx_abort, mut rx_abort) = futures::sync::oneshot::channel();
        thread::spawn(move || {
            loop {
                if let Some(exit_status) = plugin.process.poll() {
                    let _ = tx_monitor.send(exit_status);
                    break;
                }
                match rx_abort.try_recv() {
                    Ok(Some(_)) | Err(_) => {
                        break;
                    }
                    _ => {}
                }
                thread::sleep(Duration::from_secs(1));
            }
            drop(plugin);
        });
        plugin_monitors.push(boxed_future(rx_monitor));
        plugin_aborts.push(tx_abort);
    }

    // Wait for any of the plugin processes to exit.
    futures_unordered(plugin_monitors)
        .into_future()
        .then(|res| -> io::Result<()> {
            // Ask all monitors to abort.
            for tx_abort in plugin_aborts {
                let _ = tx_abort.send(1);
            }
            // Wait for monitors to complete.
            thread::sleep(Duration::from_secs(1));
            match res {
                Ok(..) => Ok(()),
                Err(..) => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to wait for plugin process",
                )),
            }
        })
}
