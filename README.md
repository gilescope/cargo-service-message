# cargo-service-message
Provides (test) service messages for Cargo for integration into CI systems like TeamCity

## How is this useful?

If you're using Teamcity or Teamcity Cloud then if you type:

`cargo service-message test` rather than `cargo test` then it should spit out service messages that TeamCity (and others?) will understand and update the UI on the fly.

## How to install it?

For now you can install it like this:
```sh
cargo install cargo-service-message --git https://github.com/gilescope/cargo-service-message.git
```

Licenced as Apache 2.0 or MIT at your choice.

## How to use it?

put it in after cargo but use the same commands as cargo. If you have a command line that doesn't work then raise an issue.

These are example commands that seem to work so far:

```sh
cargo service-message test --all-targets
```

```sh
cargo service-message clippy
```


## Tracking Issues:

https://github.com/rust-lang/rust/issues/49359

https://github.com/rust-lang/rust/issues/50297