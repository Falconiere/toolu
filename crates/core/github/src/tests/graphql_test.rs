use serde_json::json;

use crate::graphql::errors;

#[test]
fn errors_are_the_messages_of_a_non_empty_list() {
  assert_eq!(
    errors(&json!({ "errors": [{ "message": "Field 'x' doesn't exist" }, {}] })),
    Some(vec![
      "Field 'x' doesn't exist".to_owned(),
      "GraphQL error".to_owned()
    ])
  );
  assert_eq!(errors(&json!({ "errors": [], "data": {} })), None);
  assert_eq!(errors(&json!({ "data": {} })), None);
}
