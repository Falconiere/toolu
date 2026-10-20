use toolu_runtime::env::Env;

use super::{Checkout, ORIGIN};

fn locate(pairs: &[(&str, &str)]) -> Result<Checkout, String> {
  let env = Env::from_pairs(pairs.iter().map(|(key, value)| (*key, (*value).to_owned())));
  Checkout::locate(&env, "8.0.4", ORIGIN).map_err(|fail| fail.message)
}

#[test]
fn the_checkout_follows_an_absolute_xdg_data_home_else_home() {
  let xdg = locate(&[("XDG_DATA_HOME", "/data"), ("HOME", "/home/u")]);
  assert_eq!(
    xdg.map(|checkout| (checkout.dir_text(), checkout.tag)),
    Ok((
      "/data/toolu/muse-marketplace".to_owned(),
      "v8.0.4".to_owned()
    ))
  );
  let relative = locate(&[("XDG_DATA_HOME", "data"), ("HOME", "/home/u")]);
  assert_eq!(
    relative.map(|checkout| checkout.dir_text()),
    Ok("/home/u/.local/share/toolu/muse-marketplace".to_owned())
  );
  assert!(locate(&[("XDG_DATA_HOME", "data")]).is_err_and(|message| message.contains("HOME")));
}

#[test]
fn git_argv_clone_the_tag_and_move_the_checkout() {
  let Ok(checkout) = locate(&[("HOME", "/h")]) else {
    panic!("no checkout");
  };
  let dir = "/h/.local/share/toolu/muse-marketplace";
  assert_eq!(
    checkout.clone_argv().join(" "),
    format!("git clone --quiet --depth 1 --branch v8.0.4 {ORIGIN} {dir}")
  );
  assert_eq!(
    checkout.fetch_argv().join(" "),
    format!("git -C {dir} fetch --quiet --depth 1 origin tag v8.0.4")
  );
  assert_eq!(
    checkout.detach_argv().join(" "),
    format!("git -C {dir} checkout --quiet --detach v8.0.4")
  );
  assert_eq!(
    checkout.has_tag_argv().join(" "),
    format!("git -C {dir} rev-parse --quiet --verify refs/tags/v8.0.4")
  );
  assert_eq!(
    checkout.origin_argv().join(" "),
    format!("git -C {dir} remote get-url origin")
  );
}

#[test]
fn ownership_is_by_path_prefix_as_written_or_resolved() {
  let Ok(checkout) = locate(&[("HOME", "/h")]) else {
    panic!("no checkout");
  };
  assert!(checkout.is("/h/.local/share/toolu/muse-marketplace"));
  assert!(!checkout.is("/h/.local/share/toolu/muse-marketplace/plugins"));
  assert!(checkout.owns("/h/.local/share/toolu/muse-marketplace/plugins/toolu"));
  assert!(!checkout.owns("/h/src/toolu/plugins/toolu"));
  assert!(!checkout.owns("/h/.local/share/toolu/muse-marketplace-old/plugins/toolu"));
}
