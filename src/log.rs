use slog::Drain;
use std::sync::Once;
use std::rc::Rc;
use std::mem::zeroed;

lazy_static! {
    static ref LOG: slog::Logger = {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();

        slog::Logger::root(drain, o!())
    };
}

pub fn get_root_logger() -> slog::Logger {
    LOG.new(o!())
}
