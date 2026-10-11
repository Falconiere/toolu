//! Assertions that return an error from integration tests using `?`.

macro_rules! ensure {
  ($condition:expr $(,)?) => {
    if !$condition {
      return Err(std::io::Error::other(concat!("check failed: ", stringify!($condition))).into());
    }
  };
  ($condition:expr, $($message:tt)+) => {
    if !$condition {
      return Err(std::io::Error::other(format!($($message)+)).into());
    }
  };
}

macro_rules! ensure_eq {
  ($left:expr, $right:expr $(,)?) => {
    if $left != $right {
      return Err(
        std::io::Error::other(concat!(
          "check failed: ",
          stringify!($left),
          " == ",
          stringify!($right)
        ))
        .into(),
      );
    }
  };
}
