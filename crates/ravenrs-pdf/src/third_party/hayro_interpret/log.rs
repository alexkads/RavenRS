//! Logging macros that optionally forward to the `log` crate.

macro_rules! error {
    ($fmt:literal $(, $($arg:expr),* $(,)?)?) => {{
        #[cfg(any())]
        {
            ::log::error!($fmt $(, $($arg),*)?);
        }
        {
            $($(let _ = &$arg;)*)?
        }
    }};
}

macro_rules! warn {
    ($fmt:literal $(, $($arg:expr),* $(,)?)?) => {{
        #[cfg(any())]
        {
            ::log::warn!($fmt $(, $($arg),*)?);
        }
        {
            $($(let _ = &$arg;)*)?
        }
    }};
}
