//! The crate links from outside and names its layer.

#[test]
fn the_crate_links_and_names_its_layer() {
  assert_eq!(toolu_github::LAYER, "github");
}
