//! `shell-writes.test.ts`: sed and perl write in place only, never their
//! script; cp, mv and install write their destination and each
//! `DEST/basename(SRC)`, since `DEST` may be a directory.

use crate::writes::tests::{paths, some};

#[test]
fn sed_and_perl_write_their_files_only_in_place() {
  assert_eq!(paths("sed -i 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(paths("sed -i '' 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(
    paths("sed -i.bak -e s/a/b/ .env other"),
    some(&[".env", "other"])
  );
  assert_eq!(paths("sed --in-place=.bak 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(paths("sed -Ei 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(paths("sed 's/a/b/' .env"), Vec::<Option<String>>::new());
  assert_eq!(paths("perl -i -pe 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(paths("perl -pi -e 's/a/b/' .env"), some(&[".env"]));
  assert_eq!(paths("perl -ne 'print' .env"), Vec::<Option<String>>::new());
}

#[test]
fn cp_mv_and_install_write_their_destination() {
  assert_eq!(
    paths("cp src/.env apps/api/.env 2>&1"),
    some(&["apps/api/.env", "apps/api/.env/.env"])
  );
  assert_eq!(
    paths("mv source.txt .env"),
    some(&[".env", ".env/source.txt"])
  );
  assert_eq!(
    paths("install -m 644 source.txt .env"),
    some(&[".env", ".env/source.txt"])
  );
  assert_eq!(paths("cp /tmp/x/.env ."), some(&[".", "./.env"]));
  assert_eq!(
    paths("cp .env.example apps/web"),
    some(&["apps/web", "apps/web/.env.example"])
  );
  assert_eq!(paths("cp -t apps/api src/.env"), some(&["apps/api/.env"]));
  assert_eq!(
    paths("cp --target-directory=apps/api/ src/.env b"),
    some(&["apps/api/.env", "apps/api/b"])
  );
  assert_eq!(
    paths("cp a/.env b/.npmrc dest"),
    some(&["dest", "dest/.env", "dest/.npmrc"])
  );
  assert_eq!(paths("cp .env dest/"), some(&["dest/", "dest/.env"]));
  assert_eq!(paths("install -d .env conf"), some(&[".env", "conf"]));
  assert_eq!(paths("/usr/bin/install -d .env"), some(&[".env"]));
  assert_eq!(paths("cp onlyone"), Vec::<Option<String>>::new());
  assert_eq!(paths("cp"), Vec::<Option<String>>::new());
}

#[test]
fn a_dynamic_destination_or_target_directory_stays_unknown() {
  assert_eq!(paths("cp a \"$D\""), [None]);
  assert_eq!(paths("cp -t \"$D\" a b"), [None, None]);
  assert_eq!(paths("cp -t"), Vec::<Option<String>>::new());
  assert_eq!(paths("mv a b $C"), [None]);
}
