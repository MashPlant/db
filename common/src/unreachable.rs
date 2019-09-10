#[macro_export]
macro_rules! debug_unreachable {
  () => (if cfg!(debug_assertions) { unreachable!(); } else { std::hint::unreachable_unchecked() })
}