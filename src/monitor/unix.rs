use super::*;
use futures::{self, Future, Stream};
use libc;
use relay::boxed_future;
use tokio;
use tokio_io::IoFuture;

pub fn create_signal_monitor() -> impl Future<Item = (), Error = io::Error> + Send {
    use tokio_signal::unix::Signal;

    // Monitor SIGCHLD, triggered if subprocess (plugin) is exited.
    let fut1 = Signal::new(libc::SIGCHLD)
        .and_then(|signal| {
            signal
                .take(1)
                .for_each(|_| -> Result<(), io::Error> {
                    error!("Plugin exited unexpectly (SIGCHLD)");
                    Ok(())
                })
                .map(|_| libc::SIGCHLD)
        })
        .map_err(|err| {
            error!("Failed to monitor SIGCHLD, err: {:?}", err);
            err
        });

    // Monitor SIGTERM, triggered if shadowsocks is exited gracefully. (Kill by user).
    let fut2 = Signal::new(libc::SIGTERM)
        .and_then(|sigterm| {
            sigterm
                .take(1)
                .for_each(|_| -> Result<(), io::Error> {
                    info!("Received SIGTERM, exiting.");
                    Ok(())
                })
                .map(|_| libc::SIGTERM)
        })
        .map_err(|err| {
            error!("Failed to monitor SIGTERM, err: {:?}", err);
            err
        });

    // Monitor SIGINT, triggered by CTRL-C
    let fut3 = Signal::new(libc::SIGINT)
        .and_then(|sigint| {
            sigint
                .take(1)
                .for_each(|_| -> Result<(), io::Error> {
                    info!("Received SIGINT, exiting.");
                    Ok(())
                })
                .map(|_| libc::SIGINT)
        })
        .map_err(|err| {
            error!("Failed to monitor SIGINT, err: {:?}", err);
            err
        });

    // Wait for any of the futures to complete.
    future::future::select_all(vec![boxed_future(fut1), boxed_future(fut2), boxed_future(fut3)]).then(|res| match res {
        Ok(..) => Ok(()),
        Err((err, ..)) => Err(err),
    })
}
