use super::*;
use futures::{self, Future, Stream};
use libc;
use relay::boxed_future;
use tokio;
use tokio_io::IoFuture;

pub fn create_signal_monitor() -> impl Future<Item = (), Error = io::Error> + Send {
    use tokio_signal::unix::Signal;

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
    fut1.select(fut2).then(|res| match res {
        Ok(..) => Ok(()),
        Err((err, ..)) => Err(err),
    })
}
