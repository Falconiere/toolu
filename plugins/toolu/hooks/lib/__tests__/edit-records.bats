#!/usr/bin/env bats

LIB="${BATS_TEST_DIRNAME}/../edit-records.sh"

setup() {
  # shellcheck source=../edit-records.sh
  [ -f "$LIB" ] && . "$LIB"
}

@test "edit records: Claude Edit remains one update record" {
  input='{"tool_name":"Edit","tool_input":{"file_path":"src/main.ts"}}'
  run toolu_normalize_edit_records "$input" Edit
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | jq -s 'length')" = 1 ]
  printf '%s\n' "$output" | jq -e '.path == "src/main.ts" and .operation == "update"' >/dev/null
}

@test "edit records: apply_patch parses add update delete and both sides of a move" {
  patch='*** Begin Patch
*** Add File: src/new file.ts
+export const x = 1
*** Update File: src/old.ts
@@
-old
+new
*** Delete File: src/gone.rs
*** Update File: src/from.ts
*** Move to: src/to.ts
@@
-before
+after
*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run toolu_normalize_edit_records "$input" apply_patch
  [ "$status" -eq 0 ]
  records=$(printf '%s\n' "$output" | jq -s .)
  [ "$(jq 'length' <<<"$records")" = 5 ]
  jq -e '.[0] == {path:"src/new file.ts",operation:"add"}' <<<"$records" >/dev/null
  jq -e '.[1] == {path:"src/old.ts",operation:"update"}' <<<"$records" >/dev/null
  jq -e '.[2] == {path:"src/gone.rs",operation:"delete"}' <<<"$records" >/dev/null
  jq -e '.[3] == {path:"src/from.ts",operation:"update",moved_to:"src/to.ts"}' <<<"$records" >/dev/null
  jq -e '.[4] == {path:"src/to.ts",operation:"move",from:"src/from.ts"}' <<<"$records" >/dev/null
}

@test "edit records: multiple ordinary update headers are all emitted in order" {
  patch='*** Begin Patch
*** Update File: a.ts
@@
-a
+b
*** Update File: b.rs
@@
-c
+d
*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run toolu_normalize_edit_records "$input" apply_patch
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | jq -s -r 'map(.path) | join(",")')" = "a.ts,b.rs" ]
}

@test "edit records: apply_patch accepts the optional End of File marker" {
  patch='*** Begin Patch
*** Update File: src/main.ts
@@
-old
+new
*** End of File
*** End Patch'
  input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
  run toolu_normalize_edit_records "$input" apply_patch
  [ "$status" -eq 0 ]
  [ "$(printf '%s\n' "$output" | jq -s -r 'map(.path) | join(",")')" = "src/main.ts" ]
}

@test "edit records: malformed apply_patch is rejected without partial records" {
  for patch in \
    $'*** Add File: a.ts\n+x\n*** End Patch' \
    $'*** Begin Patch\n*** Update File: a.ts' \
    $'*** Begin Patch\n*** Move to: b.ts\n*** End Patch' \
    $'*** Begin Patch\n*** Add File: \n*** End Patch' \
    $'*** Begin Patch\n*** End Patch'; do
    input=$(jq -cn --arg command "$patch" '{tool_name:"apply_patch",tool_input:{command:$command}}')
    run toolu_normalize_edit_records "$input" apply_patch
    [ "$status" -eq 2 ]
    [ -z "$output" ]
  done
}

@test "edit records: non-edit tools are not normalized" {
  input='{"tool_name":"Bash","tool_input":{"command":"true"}}'
  run toolu_normalize_edit_records "$input" Bash
  [ "$status" -eq 1 ]
  [ -z "$output" ]
}
