# cargo-service-message

Provides (test) service messages for Cargo for integration into CI systems like TeamCity.

(See also https://github.com/JetBrains/teamcity-rust-plugin if you are allowed to add plugins to your TeamCity instance)

## How to install it?

For now you can install it like this:
```sh
cargo install cargo-service-message --git https://github.com/gilescope/cargo-service-message.git
```

If you want coverage also install grcov:
```
cargo install grcov
```

## How to use it?

If you're using Teamcity or Teamcity Cloud then if you type:

`cargo service-message test` rather than `cargo test` then it should spit out service messages that TeamCity (and others?) will understand and update the UI on the fly.

If you have a command line that doesn't work then please raise an issue.

These are example commands that seem to work so far:

## What's supported out of the box:

   * cargo service-message test (test results appear in the TeamCity UI as they happen)
   * cargo service-message bench (stats logged so TeamCity can graph them)
   * cargo service-message clippy (violations appear as inspections)
   * cargo service-message build (warnings appear as inspections)
   * cargo service-message check
   * cargo service-message rustc
   * cargo service-message clean (no-op passthrough)
   * cargo service-message fmt (no-op passthrough)

For compiles it will add in cargo-timings.html to the artifacts. I can't configure the report tab to display it for you - you can do that from the root project for all projects in the instance and if the report is there it will add the tab.

set env SERVICE_MESSAGE="--cover" for coverage to be generate.

set env SERVICE_MESSAGE="--debug" for debug messages.

## Todo list:
   [ ] Style coverage results so they don't look dreadful.

   [ ] Write some more tests now coverage is automatic.

   [ ] If the subcommand causes problems, we could read from stdin E.g. `cargo test --message-format=json | cargo-service-message`

   [ ] Support just/cargo-make?

## Teamcity Todo list:

Hi Jetbrains, here are the things that would make Teamcity + Rust even more awesome:

   [ ] Use monospaced font in inspections.

   [ ] Interpret ansi escape codes in inspections as happens in the teamcity build log.

## Help Wanted:

   [ ] TeamCity playtesters required - try it on your teamcity instance and give us feedback!

   [ ] Hackers required - help make the crate production ready and able to do the right thing in as many circumstances as possible.

## Licence:

Licenced as Apache 2.0 or MIT at your choice.

## Tracking Issues:

https://github.com/rust-lang/rust/pull/77890

https://github.com/rust-lang/rust/issues/49359

https://github.com/rust-lang/rust/issues/50297

## Release log:

0.1.6 (Unreleased): Honor CARGO_TARGET_DIR

0.1.5 Ignore 3rd party crates in coverage.

0.1.4 Initial crates release