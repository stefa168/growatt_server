pub trait LogErrorExt {
    /// Logs the error via `tracing::error!` if present, then passes the
    /// `Result` through unchanged. Drop-in replacement for the old
    /// `log_error!` macro.
    fn log_error(self) -> Self;
}

impl<T, E: std::fmt::Debug> LogErrorExt for Result<T, E> {
    #[track_caller]
    fn log_error(self) -> Self {
        let caller = std::panic::Location::caller();
        self.inspect_err(|e| tracing::error!(at = %caller, "{e:?}"))
    }
}
