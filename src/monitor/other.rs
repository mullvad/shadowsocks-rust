use super::*;

pub fn create_signal_monitor() -> impl Future<Item = (), Error = io::Error> + Send {
    // FIXME: What can I do ...
    // Blocks forever
    futures::empty::<(), ()>().and_then(|_| {})
}
