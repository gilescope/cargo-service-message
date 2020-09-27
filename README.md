# cargo-service-message
Provides (test) service messages for Cargo for integration into CI systems like TeamCity

## How is this useful?

If you're using Teamcity or Teamcity Cloud then if you type:

`cargo service-message test` rather than `cargo test` then it should spit out service messages that TeamCity (and others?) will understand and update the UI on the fly.

## How to install it?

For now you can install it like this:
```sh
cargo install --git https://github.com/gilescope/cargo-service-message.git
```

Licenced as Apache 2.0 or MIT at your choice.
