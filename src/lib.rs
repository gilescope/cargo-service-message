//use serde_json::{Deserializer};

// Sometimes it's important to have some doc tests.
// ```
// assert!(true);
// ```
// #[derive(Deserialize, Debug)]
// #[serde(tag="type")]
// pub enum Event {
//     #[serde(alias = "suite")]
//     Suite {
//         event: String,
//         test_count: Option<i32>,
//         passed: Option<i32>,
//         failed: Option<i32>,
//         allowed_fail: Option<i32>,
//         ignored: Option<i32>,
//         measured: Option<i32>,
//         filtered_out: Option<i32>
//     },
//     #[serde(alias = "test")]
//     Test {
//         event: String,
//         name: String,
//         exec_time: Option<String>,
//         stdout: Option<String>
//     }
// }

// {"reason":"compiler-artifact",
// "package_id":"proc-macro2 1.0.23 (registry+https://github.com/rust-lang/crates.io-index)",
// "target":{
//     "kind":["custom-build"],
//     "crate_types":["bin"],
//     "name":"build-script-build",
//     "src_path":"/Users/gilescope/.cargo/registry/src/github.com-1ecc6299db9ec823/proc-macro2-1.0.23/build.rs",
//     "edition":"2018",
//     "doctest":false
// },
// "profile":
// {
//     "opt_level":"0",
//     "debuginfo":2,
//     "debug_assertions":true,
//     "overflow_checks":true,
//     "test":false
// },
// "features":["default","proc-macro"],
// "filenames":["/Users/gilescope/projects/cargo-service-message2/target/debug/build/proc-macro2-55641086282daa27/build-script-build",
// "/Users/gilescope/projects/cargo-service-message2/target/debug/build/proc-macro2-55641086282daa27/build-script-build.dSYM"],
// "executable":null,
// "fresh":true

// {"reason":"compiler-message",
// "package_id":"cargo-service-message 0.1.1 (path+file:///Users/gilescope/projects/cargo-service-message2)",
// "target":{
//     "kind":["bin"],
//     "crate_types":["bin"],
//     "name":"cargo-service-message",
//     "src_path":"/Users/gilescope/projects/cargo-service-message2/src/bin/cargo-service-message.rs",
//     "edition":"2018",
//     "doctest":false
// },
// "message":
// {
//     "rendered":"warning: unused `#[macro_use]` import\n --> src/bin/cargo-service-message.rs:1:1\n  |\n1 | #[macro_use]\n  | ^^^^^^^^^^^^\n  |\n  = note: `#[warn(unused_imports)]` on by default\n\n",
//     "children":
//     [
//         {"children":[],
//         "code":null,
//         "level":"note",
//         "message":"`#[warn(unused_imports)]` on by default",
//         "rendered":null,
//         "spans":[]}
//     ],
//     "code":
//     {
//         "code":"unused_imports",
//         "explanation":null
//     },
//     "level":"warning",
//     "message":"unused `#[macro_use]` import",
//     "spans":[
//         {
// "byte_end":12,
// "byte_start":0,
// "column_end":13,
// "column_start":1,
//         "expansion":null,
//          "file_name":"src/bin/cargo-service-message.rs",
//         "is_primary":true,"label":null,"line_end":1,"line_start":1,
//         "suggested_replacement":null,"suggestion_applicability":null,
//         "text":[
//             {"highlight_end":13,"highlight_start":1,"text":"#[macro_use]"}
//         ]
//     }
// ]
//     }
// }

// {"duration": Number(1.271503997),
// "mode": String("test"),
//  "package_id": String("cargo-service-message 0.1.1 (path+file:///Users/gilescope/projects/cargo-service-message2)"),
//   "reason": String("timing-info"),
//   "rmeta_time": Number(1.271501631),
//   "target": Object(
//       {"crate_types": Array([String("bin")]),
//          "doctest": Bool(false),
//          "edition": String("2018"),
//          "kind": Array([String("bin")]),
//           "name": String("cargo-service-message"),
//               "src_path": String("/Users/gilescope/projects/cargo-service-message2/src/bin/cargo-service-message.rs")}
//)}
