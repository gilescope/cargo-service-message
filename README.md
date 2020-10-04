# cargo-service-message
Provides (test) service messages for Cargo for integration into CI systems like TeamCity

## How to install it?

For now you can install it like this:
```sh
cargo install cargo-service-message --git https://github.com/gilescope/cargo-service-message.git
```

## How to use it?

If you're using Teamcity or Teamcity Cloud then if you type:

`cargo service-message test` rather than `cargo test` then it should spit out service messages that TeamCity (and others?) will understand and update the UI on the fly.

If you have a command line that doesn't work then please raise an issue.

These are example commands that seem to work so far:

```sh
cargo service-message test --all-targets
```

```sh
cargo service-message clippy
```

For compiles it will add in cargo-timings.html to the artifacts. I can't configure the report tab to display it for you - but on the plus side you can do that from the root project for all projects in the instance.

TODO:
   * Statistics seem to graph fine on the build results / parameters page, but not on the stats graphs page so something's not quite right with them...
   * Coverage
   * Refactor code to not be one large function :-)

## Licence:

Licenced as Apache 2.0 or MIT at your choice.

## Tracking Issues:

https://github.com/rust-lang/rust/issues/49359

https://github.com/rust-lang/rust/issues/50297