#![feature(test)]
extern crate test;

use serde_json::{Deserializer, Map, Value};
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn main() -> Result<(), String> {
    let options: Vec<String> = std::env::args().collect();
    println!("{:?}", &options);
    if let Ok(exit_code) = cargo_service_message(options) {
        std::process::exit(exit_code);
    } else {
        std::process::exit(-1);
    }
}

fn cargo_service_message(argv: Vec<String>) -> Result<i32, String> {
    if argv.len() < 2 {
        return Err(format!("Usage: 'test' as the next argument followed by the standard cargo test arguments. Found {:?}", argv));
    }
    if argv[1] != *"service-message" {
        return Err(format!("expected 'service-message' as the next argument followed by the standard cargo test arguments but got {}", argv[1]));
    }

    let exit_code = run_tests(&argv[2..]).unwrap();
    Ok(exit_code)
}

/*
{ "type": "test", "event": "started", "name": "tests::test" }
{ "type": "test", "event": "started", "name": "tests::test_fast" }
{ "type": "test", "event": "started", "name": "tests::test_slow" }

{ "type": "test", "event": "ok", "name": "tests::test", "exec_time": "0.000s" }
{ "type": "test", "event": "ok", name": "tests::test_fast", "exec_time": "0.000s" }
{ "type": "test", "event": "ok", "name": "tests::test_slow", "exec_time": "10.000s" }
{ "type": "bench", "name": "tests::example_bench_add_two", "median": 57, "deviation": 9 }
{ "type": "suite", "event": "started", "test_count": 3 }
{"event": "ignored", "name": "tests::test_a_failure_fails", "type": "test"}
{ "type": "suite", "event": "ok", "passed": 3, "failed": 0, "allowed_fail": 0, "ignored": 0, "measured": 0, "filtered_out": 0 }
*/

fn run_tests(args: &[String]) -> Result<i32, Box<dyn Error>> {
    //Params:
    let debug = std::env::var("SERVICE_FLAGS").is_ok();
    let coverage = true;
    let colors = false; //TODO wait for teamcity inspections to understand ansi
                        //Also TODO: replace ansi yellow => orange as yellow on white unreadable!
    let brand = std::env::var("SERVICE_BRAND").unwrap_or_else(|_| "teamcity".to_owned());
    let min_threshold = 5.; // Any crate that compiles faster than this many seconds won't be tracked.

    let mut cmd = Command::new("cargo");
    cmd.stderr(Stdio::inherit());
    cmd.stdout(Stdio::piped());
    cmd.args(args);

    let cargo_cmd = args[0].clone(); //TODO support +nightly

    //Even though cargo clean doesn't do json at the moment it would be good if
    // adding service-message was a no_op.
    if cargo_cmd != "clean" && cargo_cmd != "fmt" {
        cmd.arg(format!(
            "--message-format={}",
            if colors {
                "json-diagnostic-rendered-ansi"
            } else {
                "json"
            }
        )); //TODO this needs to be before --
        cmd.arg("-Ztimings=json,html,info");
    }

    if !contains("--", args) {
        cmd.arg("--");
    }

    if cargo_cmd == "test" || cargo_cmd == "bench" {
        if !contains("-Zunstable-options", args) && !contains("unstable-options", args) {
            cmd.arg("-Zunstable-options");
        }
        cmd.arg("--format");
        cmd.arg("json");
    }
    if coverage && (cargo_cmd == "test" || cargo_cmd == "build") {
        //         export CARGO_INCREMENTAL=0
        // export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort"
        // export RUSTDOCFLAGS="-Cpanic=abort"
        //cmd.arg("-Zinstrument-coverage");
        //-Zexperimental-coverage for branch level coverage but a little inaccurate at the moment.
    }

    println!("spawning: {:?}", &cmd);
    let mut child = cmd.spawn()?;
    let out_stream = Option::take(&mut child.stdout).unwrap();
    let buf = BufReader::new(out_stream);
    let x = String::new();
    let mut inspection_logged = false;

    for line in buf.lines() {
        if let Ok(ref line) = line {
            let stream = Deserializer::from_str(&line);
            for value in stream.into_iter() {
                //  println!("{:?}", &value);
                match value {
                    Ok(Value::Object(event)) => {
                        let ctx = Context {
                            debug,
                            brand: brand.to_string(),
                            min_threshold,
                        };
                        if let Ok(reported) = process(&event, &ctx) {
                            if reported {
                                inspection_logged = true;
                            }
                        }
                    }
                    Ok(_) => {
                        println!("error parsing cargo output: {}", line);
                    }
                    Err(_) => {
                        println!("{}", line);
                    }
                }
            }
        } else {
            print!("{:?}", line);
        }
    }
    //TODO only if file exists?
    println!(
        "##{}[publishArtifacts '{}']",
        brand,
        std::env::current_dir()
            .unwrap()
            .join("cargo-timing.html")
            .into_os_string()
            .into_string()
            .unwrap()
    );

    Ok(child.wait()?).map(|exit_status| {
        if let Some(exit_code) = exit_status.code() {
            // Tests and Clippy fail the build with non-zero exit codes if there's failures.
            // Better to have it return success and let people have
            // a TeamCity rule to fail if > 0 inspections.
            if inspection_logged && (cargo_cmd == "clippy" || cargo_cmd == "test") {
                0
            } else {
                exit_code
            }
        } else {
            -1
        }
    })
}

struct Context {
    debug: bool,
    brand: String,
    min_threshold: f64,
}

///Returns true if inspection was rasied.
fn process(event: &Map<String, Value>, ctx: &Context) -> Result<bool, Box<dyn Error>> {
    let brand = &ctx.brand;
    let mut inspection_logged = false;
    if let Some(Value::String(compiler_msg)) = event.get("reason") {
        match compiler_msg.as_ref() {
            "timing-info" => {
                parse_timing_info(ctx, event);
            }
            "build-script-executed" => {
                if let Some(Value::String(package_id)) = event.get("package_id") {
                    // Shame build scripts that run:
                    println!("Running build script for {}", tidy_package_id(package_id));
                }
            }
            "compiler-artifact" => {
                let fresh = if let Some(Value::Bool(fresh)) = event.get("fresh") {
                    *fresh
                } else {
                    false
                };
                if let Some(Value::String(package_id)) = event.get("package_id") {
                    // Shame build scripts that run:
                    println!(
                        "Compiling {} {}",
                        tidy_package_id(package_id),
                        if fresh { "[fresh]" } else { "" }
                    );
                }
            }
            "compiler-message" => {
                if let Some(Value::Object(msg)) = event.get("message") {
                    if let Ok(true) = parse_compiler_message(ctx, msg) {
                        inspection_logged = true;
                    }
                }
            }
            "build-finished" => {}
            _ => {
                println!("{}", compiler_msg);
                println!("{:?}", event);
            }
        }
    } else if let Some(Value::String(ttype)) = event.get("type") {
        match ttype.as_ref() {
            "suite" => match event.get("event") {
                Some(Value::String(event_name)) => match event_name.as_ref() {
                    "started" => {
                        println!("##{}[testSuiteStarted name='rust_test_suite' flowId='test_suite_flow_id']", brand);
                    }
                    "ok" => {
                        println!("##{}[testSuiteFinished name='rust_test_suite' flowId='test_suite_flow_id']", brand);
                    }
                    "failed" => {
                        inspection_logged = true;
                        println!("##{}[testSuiteFinished name='rust_test_suite' flowId='test_suite_flow_id']", brand);
                    }
                    _ => {
                        println!("format unknown {:?}", event);
                    }
                },
                _ => {
                    println!("format {:?}", event);
                }
            },
            "bench" => {
                let name = parse_name(&event);

                if let Some(Value::Number(median)) = event.get("median") {
                    println!(
                        "##{}[buildStatisticValue key='bench.{}.median' value='{:.6}']",
                        brand,
                        name,
                        median.as_f64().unwrap()
                    );
                }
                if let Some(Value::Number(devation)) = event.get("deviation") {
                    println!(
                        "##{}[buildStatisticValue key='bench.{}.deviation' value='{:.6}']",
                        brand, name, devation
                    );
                }
            }
            "test" => match event.get("event") {
                Some(Value::String(s)) => {
                    return parse_test_event(ctx, s, event);
                }
                _ => {
                    println!("unhandled event - please report: {:?}", event);
                }
            },
            _ => {
                println!("unhandled event - please report: {:?}", event);
            }
        }
    }
    Ok(inspection_logged)
}

fn parse_compiler_message(ctx: &Context, msg: &Map<String, Value>) -> Result<bool, Box<dyn Error>> {
    if let Some(Value::String(level)) = msg.get("level") {
        if level.as_str() == "warning" || level.as_str() == "error" {
            let message = if let Some(Value::String(message)) = msg.get("rendered") {
                message.as_ref()
            } else {
                ""
            };

            //TODO ask jetbrains if there's a way we can embed html here as message could
            // do with being monospaced.
            // let message = "<pre>".to_string() + &message + "</pre>";

            if let Some(Value::Object(code)) = msg.get("code") {
                let explanation = if let Some(Value::String(explanation)) = code.get("explanation")
                {
                    explanation.as_ref()
                } else {
                    "no explanation"
                };

                let code = if let Some(Value::String(code)) = code.get("code") {
                    code.as_ref()
                } else {
                    "other"
                };

                let mut file = "";
                let mut line = 0u64;
                if let Some(Value::Array(spans)) = msg.get("spans") {
                    if let Value::Object(span) = &spans[0] {
                        if let Some(Value::String(file_name)) = span.get("file_name") {
                            file = &file_name;
                        }
                        if let Some(Value::Number(line_number)) = span.get("line_start") {
                            line = line_number.as_u64().unwrap_or(0);
                        }
                    }
                }

                println!("{}", message);
                println!(
                    "##{}[inspectionType id='{}' category='warning' name='{}' description='{}']",
                    ctx.brand, code, code, explanation
                );
                println!(
                    "##{}[inspection typeId='{}' message='{}' file='{}' line='{}' SEVERITY='{}']",
                    ctx.brand,
                    code,
                    escape_message(message),
                    file,
                    line,
                    level
                );
                //additional attribute='<additional attribute>'
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    } else {
        println!("{:?}", msg);
        Ok(false)
    }
}

fn parse_test_event(
    ctx: &Context,
    event_type: &str,
    event: &Map<String, Value>,
) -> Result<bool, Box<dyn Error>> {
    //TODO split parsing from output!
    let name = parse_name(&event);

    match event_type {
        "started" => {
            println!(
                "##{}[flowStarted flowId='{}' parent='test_suite_flow_id']",
                ctx.brand, name
            );
            println!("##{}[testStarted flowId='{}' name='{}' captureStandardOutput='true' parent='test_suite_flow_id']", ctx.brand, name, name);
            Ok(false)
        }
        "ok" => {
            if let Some(exec_time) = event.get("exec_time") {
                println!(
                    "##{}[testFinished flowId='{}' name='{}' duration='{}']",
                    ctx.brand, name, name, exec_time
                );
            } else {
                println!(
                    "##{}[testFinished flowId='{}' name='{}']",
                    ctx.brand, name, name
                );
            }
            println!("##{}[flowFinished flowId='{}']", ctx.brand, name);
            Ok(false)
        }
        "ignored" => {
            //todo maybe don't ignore the ignored tests?
            Ok(false)
        }
        "failed" => {
            let stdout = if let Some(Value::String(stdout)) = event.get("stdout") {
                stdout
            } else {
                ""
            };
            if let Some((left, right)) = find_comparison(stdout) {
                println!("##{}[testFailed type='comparisonFailure' name='{}' flowId='{}' message='test failed' details='{}' expected='{}' actual='{}']", ctx.brand, name, name, escape_message(stdout),
                escape_message(left),escape_message(right));
            } else {
                println!(
                    "##{}[testFailed name='{}' flowId='{}' message='test failed' details='{}']",
                    ctx.brand,
                    name,
                    name,
                    escape_message(stdout)
                );
            }

            println!(
                "##{}[testFinished flowId='{}' name='{}']",
                ctx.brand, name, name
            );
            println!("##{}[flowFinished flowId='{}']", ctx.brand, name);
            Ok(true)
        }
        _ => {
            println!("failed to parse {:?}", event);
            Ok(false)
        }
    }
}

fn parse_timing_info(ctx: &Context, event: &Map<String, Value>) {
    if ctx.debug {
        println!("{:?}", &event);
    }
    let name = if let Some(Value::Object(target)) = event.get("target") {
        if let Some(Value::String(target_name)) = target.get("name") {
            target_name.to_string()
        } else {
            "anon".to_string()
        }
    } else {
        "anon".to_string()
    };

    let mode = if let Some(Value::String(compile_mode)) = event.get("mode") {
        compile_mode.to_string()
    } else {
        "mode".to_string()
    };

    if let Some(Value::Number(duration)) = event.get("duration") {
        if let Some(duration) = duration.as_f64() {
            if duration > ctx.min_threshold {
                println!(
                    "##{}[buildStatisticValue key='{} {}' value='{:.6}']",
                    ctx.brand, mode, name, duration
                );

                println!("Compiled {} in {:.2}s", name, duration);
            }
        }
    }
}
fn parse_name(event: &Map<String, Value>) -> String {
    let name = if let Some(Value::String(name)) = event.get("name") {
        name
    } else {
        "no_name"
    };
    name.replace("::", ".")
}

fn escape_message(unescaped: &str) -> String {
    //TODO:\uNNNN (unicode symbol with code 0xNNNN)  as |0xNNNN
    unescaped
        .replace("|", "||")
        .replace("[", "|[")
        .replace("]", "|]")
        .replace("\n", "|n")
        .replace("\r", "|r")
        .replace("'", "|'")
}

fn tidy_package_id(package_id: &str) -> String {
    package_id
        .replace(
            "(registry+https://github.com/rust-lang/crates.io-index)",
            "",
        )
        .replace(
            "(registry+https://github.com/rust-lang/crates.io-index.git)",
            "",
        )
}

fn contains(needle: &str, args: &[String]) -> bool {
    args.iter().any(|x| x == needle)
}

fn find_comparison(msg: &str) -> Option<(&str, &str)> {
    if let Some(index) = msg.find("left: `") {
        if let Some(index_end) = msg[index..].find("`,") {
            let left_end = index + index_end;
            let left = &msg[(index + "left: `".len())..left_end];

            if let Some(right_index) = msg.find("right: `") {
                if let Some(right_end) = msg[right_index..].find("`', ") {
                    let right = &msg[(right_index + "right: `".len())..(right_index + right_end)];
                    return Some((left, right));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn example_bench_add_two(b: &mut Bencher) {
        b.iter(|| {
            print!("hi");
        });
    }

    #[test]
    fn test() {
        assert!(cargo_service_message(vec!["path/to/bin".into()]).is_err());
        assert!(cargo_service_message(vec!["path/to/bin".into(), "fred".to_string()]).is_err());
        //assert_eq!(Ok(()), cargo_service_message(vec!["path/to/bin".into(),"test".to_string()]));
    }

    #[test]
    fn test_a_failure_fails() {
        assert_eq!("red", "green");
    }

    #[test]
    fn test_slow() {
        std::thread::sleep(std::time::Duration::new(1, 0));
    }

    #[test]
    fn test_fast() {
        std::thread::sleep(std::time::Duration::new(0, 20));
    }

    //Spec: if we call something that fails we should pass on the error message.

    #[test]
    fn test_compare() {
        let output = r#"
        [tests.test_a_failure_fails] thread 'tests::test_a_failure_fails' panicked at 'assertion failed: `(left == right)`
  left: `"red"`,
 right: `"green"`', src/bin/cargo-service-message.rs:194:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
        "#;

        assert_eq!(Some(("\"red\"", "\"green\"")), find_comparison(output));
    }
}
