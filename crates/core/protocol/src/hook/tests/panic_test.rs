use super::{QUIET, catch};

#[test]
fn a_value_passes_through() {
  assert_eq!(catch(|| 7), Ok(7));
}

#[test]
fn a_str_or_string_panic_becomes_its_message() {
  assert_eq!(catch(|| -> u8 { panic!("boom") }), Err("boom".to_owned()));
  let detail = "walk exploded";
  assert_eq!(
    catch(|| -> u8 { panic!("{detail}") }),
    Err("walk exploded".to_owned())
  );
}

#[test]
fn a_non_text_payload_gets_a_fixed_message() {
  assert_eq!(
    catch(|| -> u8 { std::panic::panic_any(7_u32) }),
    Err("a panic with a non-text payload".to_owned())
  );
}

#[test]
fn the_thread_reports_panics_again_after_a_catch() {
  assert!(catch(|| -> u8 { panic!("quiet") }).is_err());
  assert!(!QUIET.with(std::cell::Cell::get));
  assert_eq!(catch(|| QUIET.with(std::cell::Cell::get)), Ok(true));
  assert!(!QUIET.with(std::cell::Cell::get));
}
