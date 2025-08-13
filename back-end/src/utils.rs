#[macro_export]
macro_rules! io_error {
    ($kind:ident, $($arg:tt)*) => {{
        std::io::Error::new(std::io::ErrorKind::$kind, format!($($arg)*))
    }};
}
pub use io_error;
