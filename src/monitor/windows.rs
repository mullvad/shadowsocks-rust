use super::*;
use futures::{self, Future, Stream};

pub fn create_signal_monitor() -> impl Future<Item = (), Error = io::Error> + Send {
    // FIXME: How to handle SIGTERM equavalent in Windows?

    use tokio_signal::windows::Event;

    // Must be wrapped with a future, because when `Event` is being created, it will spawn
    // a task into the current executor. So if there is no current running executor, thread
    // will panic.
    let events = futures::lazy(|| {
        let fut1 = Event::ctrl_c()
            .and_then(|ev| {
                ev.take(1).for_each(|_| -> Result<(), io::Error> {
                    error!("Received Ctrl-C event");
                    Ok(())
                })
            })
            .map_err(|err| {
                error!("Failed to monitor Ctrl-C event: {:?}", err);
                err
            });

        let fut2 = Event::ctrl_break()
            .and_then(|ev| {
                ev.take(1).for_each(|_| -> Result<(), io::Error> {
                    error!("Received Ctrl-Break event");
                    Ok(())
                })
            })
            .map_err(|err| {
                error!("Failed to monitor Ctrl-Break event: {:?}", err);
                err
            });

        // Wait for any of the futures to complete.
        fut1.select(fut2).then(|res| match res {
            Ok(..) => Ok(()),
            Err((err, ..)) => Err(err),
        })
    });

    events
}
