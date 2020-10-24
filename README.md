# cargo-service-message

Provides (test) service messages for Cargo for integration into CI systems like TeamCity.

(See also https://github.com/JetBrains/teamcity-rust-plugin if you are allowed to add plugins to your TeamCity instance)

## How to install it?

Install the standard way:
```sh
cargo install cargo-service-message
```

If you want coverage also install grcov:
```
cargo install grcov
```

## How to use it?

If you're using Teamcity or Teamcity Cloud then if you type:

`cargo service-message test` rather than `cargo test` then it will emit service messages that TeamCity (and others?) understand and use to update their UI on the fly.

If you have a command line that doesn't work then please raise an issue.

## What's supported out of the box:

These are example commands that seem to work so far:

   * cargo service-message test (test results appear in the TeamCity UI as they happen)
   * cargo service-message bench (stats logged so TeamCity can graph them)
   * cargo service-message clippy (violations appear as inspections)
   * cargo service-message build (warnings appear as inspections)
   * cargo service-message check
   * cargo service-message rustc
   * cargo service-message clean (no-op passthrough)
   * cargo service-message fmt (no-op passthrough)

For compiles it will add in cargo-timings.html to the artifacts. I can't configure the report tab to display it for you - you can do that from the root project for all projects in the instance and if the report is there it will add the tab.

set env SERVICE_MESSAGE="--cover" for coverage to be generated.

If you do not wish for the coverage report to be generated after that invocation (because you have some more
tests to run that will influence the coverage) then use: "--cover-without-report".

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

   [ ] Hackers required - help make the crate do the right thing in as many circumstances as possible.

## Licence:

Licenced as Apache 2.0 or MIT at your choice.

## Tracking Issues:

https://github.com/rust-lang/rust/pull/77890

https://github.com/rust-lang/rust/issues/49359

https://github.com/rust-lang/rust/issues/50297

## Release log:

0.1.8 Report ignored tests.

0.1.7 (Unreleased): Have coverage work with proc-macros.

0.1.6 (Unreleased): Honor CARGO_TARGET_DIR

0.1.5 Ignore 3rd party crates in coverage.

0.1.4 Initial crates release