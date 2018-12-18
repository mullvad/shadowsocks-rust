use super::Plugin;
use futures::{self, stream::futures_unordered, Future, Stream};
use relay::boxed_future;
use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

pub fn create_plugin_monitor(
    plugins: Vec<Plugin>,
    abort_signal: Arc<AtomicBool>,
) -> impl Future<Item = (), Error = io::Error> + Send {
    if plugins.is_empty() {
        panic!("Cannot monitor zero plugins");
    }

    let mut plugin_monitors = Vec::new();

    for mut plugin in plugins {
        let (tx_monitor, rx_monitor) = futures::sync::oneshot::channel();
        let thread_abort_signal = abort_signal.clone();
        thread::spawn(move || {
            loop {
                if let Ok(Some(exit_status)) = plugin.process.wait_timeout(Duration::from_secs(1)) {
                    let _ = tx_monitor.send(exit_status);
                    break;
                }
                if thread_abort_signal.load(Ordering::Relaxed) {
                    let _ = plugin.process.terminate();
                }
            }
        });
        plugin_monitors.push(boxed_future(rx_monitor));
    }

    // Wait for the first plugin process to exit.
    let f = futures_unordered(plugin_monitors).into_future().then(move |res| {
        // Signal all monitoring threads to abort.
        abort_signal.store(true, Ordering::Relaxed);

        let remaining = match res {
            Ok((_item, remaining)) => remaining,
            Err((_error, remaining)) => remaining,
        };

        // Wait on all remaining monitoring threads.
        remaining
            .then(|_res| Ok(()))
            .collect()
            .map(|_| ())
            .map_err(|()| io::Error::new(io::ErrorKind::Other, "Placeholder"))
    });

    boxed_future(f)
}
