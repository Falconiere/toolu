#!/usr/bin/env bats
# Acceptance tests for detect-stack.sh against REAL upstream manifests.
# Fixtures + their provenance live under fixtures/ (see fixtures/PROVENANCE.md).
load helpers

# AC3: a real Next.js + Tailwind app -> web, react + next, tailwind.
@test "web-next: real Next+Tailwind package.json -> platform web with react/next/tailwind" {
  detect "$FIXTURES/web-next"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"platform": "web"'* ]]
  [[ "$output" == *'"react"'* ]]
  [[ "$output" == *'"next"'* ]]
  [[ "$output" == *'"tailwind"'* ]]
}

# AC4: a real Flutter pubspec.yaml -> mobile, flutter true.
@test "mobile-flutter: real Flutter pubspec.yaml -> platform mobile with flutter true" {
  detect "$FIXTURES/mobile-flutter"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"platform": "mobile"'* ]]
  [[ "$output" == *'"flutter": true'* ]]
}

# AC5: a real Expo (create-expo-app) app -> mobile, expo + reactNative true.
@test "mobile-expo: real Expo app -> platform mobile with expo and reactNative true" {
  detect "$FIXTURES/mobile-expo"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"platform": "mobile"'* ]]
  [[ "$output" == *'"expo": true'* ]]
  [[ "$output" == *'"reactNative": true'* ]]
}

# AC6: an undetectable/empty project -> unknown, AND exit 0.
@test "empty: undetectable project -> platform unknown and exit 0" {
  detect "$FIXTURES/empty"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"platform": "unknown"'* ]]
}

# AC7: --no-jq (pure-bash path) yields identical platform + frameworks to the
# default (jq) path on the same real fixture.
@test "no-jq: pure-bash path matches jq path on web-next (platform + frameworks)" {
  detect "$FIXTURES/web-next"
  [ "$status" -eq 0 ]
  jq_platform="$(printf '%s\n' "$output" | grep '"platform"')"
  jq_frameworks="$(printf '%s\n' "$output" | grep '"frameworks"')"

  detect --no-jq "$FIXTURES/web-next"
  [ "$status" -eq 0 ]
  nojq_platform="$(printf '%s\n' "$output" | grep '"platform"')"
  nojq_frameworks="$(printf '%s\n' "$output" | grep '"frameworks"')"

  [ "$jq_platform" = "$nojq_platform" ]
  [ "$jq_frameworks" = "$nojq_frameworks" ]
}

# Valid-JSON check: the default --json output must parse as JSON.
@test "json: --json output for web-next parses as valid JSON" {
  detect --json "$FIXTURES/web-next"
  [ "$status" -eq 0 ]
  printf '%s' "$output" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{JSON.parse(s);process.exit(0)})'
}

# Failure path: an unknown flag exits 2 (not 0) and names the offending flag —
# bad usage is the only non-zero exit (undetectable projects still exit 0).
@test "bad-flag: unknown flag exits 2 and names it" {
  detect --bogus
  [ "$status" -eq 2 ]
  [[ "$output" == *"unknown flag '--bogus'"* ]]
}

# Portability: the pure-bash (--no-jq) path must run under a stripped environment
# with ONLY BSD /usr/bin tools and /bin/bash (3.2) — no jq, no GNU coreutils —
# and still detect the real Next+Tailwind fixture as web. Restricting the
# environment is allowed (it's not mocking the data under test).
@test "portability: --no-jq runs under env -i with BSD tools and bash 3.2" {
  run env -i PATH=/usr/bin:/bin bash "$DETECTOR" --no-jq "$FIXTURES/web-next"
  [ "$status" -eq 0 ]
  [[ "$output" == *'"platform": "web"'* ]]
  [[ "$output" == *'"react"'* ]]
  [[ "$output" == *'"next"'* ]]
}
