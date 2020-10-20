use serde_json::{Deserializer, Map, Value};
use std::env;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), String> {
    //Setup interrupt handling (TODO: not sure this is actually responding to teamcity stop events?)
    thread::spawn(|| {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::new(0, 500));
        }
        std::process::exit(-1);
    });

    let options: Vec<String> = std::env::args().collect();
    println!("{:?}", &options);

    if let Ok(exit_code) = cargo_service_message(options) {
        std::process::exit(exit_code);
    } else {
        std::process::exit(-1);
    }
}

fn cargo_service_message(argv: Vec<String>) -> Result<i32, String> {
    if env::var("SERVICEMESSAGE").is_ok() {
        eprintln!("env var SERVICEMESSAGE set but should be SERVICE_MESSAGE");
    }
    if argv.len() < 2 {
        return Err(format!("Usage: 'test' as the next argument followed by the standard cargo test arguments. Found {:?}", argv));
    }
    if argv[1] != *"service-message" {
        return Err(format!("expected 'service-message' as the next argument followed by the standard cargo test arguments but got {}", argv[1]));
    }

    let exit_code = run_cargo(&argv[2..]).unwrap();
    Ok(exit_code)
}

#[cfg(not(target_os = "windows"))]
const fn default_cargo_home() -> &'static str {
    "HOME"
}

#[cfg(target_os = "windows")]
const fn default_cargo_home() -> &'static str {
    "USERPROFILE"
}

fn cargo_home() -> Result<String, std::env::VarError> {
    env::var("CARGO_HOME").or_else(|_| {
        env::var(default_cargo_home()).map(|mut home| {
            home.push_str("/.cargo");
            home
        })
    })
}

fn run_cargo(args: &[String]) -> Result<i32, Box<dyn Error>> {
    //Params:
    let debug = std::env::var("SERVICE_MESSAGE")
        .unwrap_or_else(|_| "".into())
        .contains("--debug");
    let mut coverage = std::env::var("SERVICE_MESSAGE")
        .unwrap_or_else(|_| "".into())
        .contains("--cover");

    let cargo_cmd = &args[0]; //TODO: support +nightly

    if coverage && cargo_cmd == "test" {
        if Command::new("grcov").arg("--version").output().is_err() {
            coverage = false;
            println!("cargo-service-message: grcov not found on path so no coverage. (cargo install grcov?)");
        } else {
            println!("testing with coverage...");
            let _clean_done = Command::new("cargo").arg("clean").status();
        }
    } else if cargo_cmd == "test" {
        println!("testing without coverage (set SERVICE_MESSAGE=--cover for coverage)");
    }
    if coverage && cargo_cmd != "test" {
        coverage = false;
    }

    let colors = false; //TODO: wait for teamcity inspections to understand ansi
                        //Also TODO: replace ansi yellow => orange as yellow on white unreadable unless in darkmode!
    let brand = std::env::var("SERVICE_BRAND").unwrap_or_else(|_| "teamcity".to_owned());
    let min_threshold = 5.; // Any crate that compiles faster than this many seconds won't be tracked via statistics.

    let mut cmd = Command::new("cargo");
    cmd.stderr(Stdio::inherit());
    cmd.stdout(Stdio::piped());
    cmd.args(args);

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
        )); //TODO: this needs to be before --
        cmd.arg("-Ztimings=json,html,info");
        //TODO: this creates cargo-timings.html and friends in the root - we should move those to the target dir.
    }

    let mode = if contains("--release", args) {
        "release"
    } else {
        "debug"
    };

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
        // TODO: maybe support -Zexperimental-coverage

        let flags = env::var("RUSTFLAGS").unwrap_or_else(|_| "".to_string());
        // TODO: dedup flags if already set
        cmd.env("RUSTFLAGS", format!("{} -Zinstrument-coverage -Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort", flags));
        cmd.env("CARGO_INCREMENTAL", "0");
        cmd.env("RUSTDOCFLAGS", "-Cpanic=abort");
    }

    println!("spawning: {:?}", &cmd);
    let mut child = cmd.spawn()?;
    let out_stream = Option::take(&mut child.stdout).unwrap();
    let buf = BufReader::new(out_stream);
    let mut inspection_logged = false;
    let ctx = Context {
        debug,
        brand: brand.to_string(),
        min_threshold,
    };

    for line in buf.lines() {
        if let Ok(ref line) = line {
            if ctx.debug {
                println!("{}", &line);
            }
            let stream = Deserializer::from_str(&line);
            for value in stream.into_iter() {
                match value {
                    Ok(Value::Object(event)) => {
                        if let Ok(reported) =
                            process(&ctx, &event, &mut std::io::stdout(), &mut std::io::stderr())
                        {
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

    let result = Ok(child.wait()?).map(|exit_status| {
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
    });

    if coverage {
        gen_coverage_report(&ctx, mode);
    }
    result
}

fn gen_coverage_report(ctx: &Context, mode: &str) {
    let target = target_dir();
    let mut grcov = grcov_cmd(
        &target.join(mode),
        "html",
        target.join("coverage").to_str().unwrap(),
    );
    println!("{:?}", grcov);
    let grcov_html_out = grcov.output();

    let json_filename: PathBuf = target.join("coverage.json");
    let mut grcov = grcov_cmd(
        &target.join(mode),
        "covdir",
        json_filename.to_str().unwrap(),
    );
    match grcov.output() {
        Ok(output) => {
            if let Ok(coverage) = std::fs::read_to_string(json_filename) {
                let (percent, lcov, _lmiss, ltot) = parse_cov(&coverage);

                println!(
                    "##{}[buildStatisticValue key='CodeCoverageL' value='{:.6}']",
                    ctx.brand, percent
                );
                println!(
                    "##{}[buildStatisticValue key='CodeCoverageAbsLCovered' value='{:.6}']",
                    ctx.brand, lcov
                );
                println!(
                    "##{}[buildStatisticValue key='CodeCoverageAbsLTotal' value='{:.6}']",
                    ctx.brand, ltot
                );
            } else {
                println!("Coverage didn't produce json file - output follows:");
                println!("{}", &String::from_utf8_lossy(&output.stdout));
                println!("{}", &String::from_utf8_lossy(&output.stderr));
                println!("-- fin --");
            }
        }
        Err(err) => println!("coverage failed to execute: {:?}", err),
    }

    if let Err(err) = grcov_html_out {
        eprintln!("grcov error while processing coverage: {}", err);
    } else {
        //An attempt to override the css file...
        //std::thread::sleep(Duration::new(1, 0));
        // use std::fs::File;
        // use std::io::Write;
        // let file_name = std::env::current_dir()
        //     .unwrap()
        //     .join("target/coverage/grcov.css");
        // println!("going to {:?}", &file_name);

        // if let Err(rr) = std::fs::remove_file(&file_name) {
        //     eprintln!("Error {:?}", rr);
        // }
        // println!("did {:?}", &file_name);
        // println!("{}", CSS);
        // {
        //     let mut f = File::create(file_name).expect("Unable to create file");
        //     f.write_all(CSS.as_bytes()).expect("Unable to write data");
        // }
        // f.drop();

        println!(
            "##{}[publishArtifacts '{}/**=>coverage.zip']",
            ctx.brand,
            target.join("coverage").to_str().unwrap()
        );
    }
}

fn target_dir() -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(|s| PathBuf::from_str(&s).unwrap())
        .unwrap_or_else(|_| std::env::current_dir().unwrap().join("target"))
}

/// Command to generate coverage
fn grcov_cmd(input_dir: &Path, output_type: &str, output_dir: &str) -> Command {
    let mut grcov = Command::new("grcov");
    grcov
        .arg(input_dir.to_str().unwrap())
        .arg("-s")
        .arg(".")
        .arg("-t")
        .arg(output_type)
        .arg("--llvm")
        .arg("--branch")
        .arg("--ignore-not-existing")
        .arg("-o")
        .arg(output_dir);

    // Ignore 3rd party crates
    if let Ok(cargo_home) = cargo_home() {
        grcov.arg("--ignore").arg(format!("{}/**", cargo_home));
    }
    grcov
}

//static CSS: &str = include_str!("grcov.css");

struct Context {
    debug: bool,
    brand: String,
    min_threshold: f64,
}

/// Processes a line of output from cargo and potentially augments that output with service messages.
/// Returns true if inspection was rasied.
fn process(
    ctx: &Context,
    event: &Map<String, Value>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, Box<dyn Error>> {
    let brand = &ctx.brand;
    let mut inspection_logged = false;
    if let Some(Value::String(compiler_msg)) = event.get("reason") {
        match compiler_msg.as_ref() {
            "timing-info" => {
                parse_timing_info(ctx, event, out, err)?;
            }
            "build-script-executed" => {
                if let Some(Value::String(package_id)) = event.get("package_id") {
                    // Shame build scripts that run:
                    writeln!(
                        out,
                        "Running build script for {}",
                        tidy_package_id(package_id)
                    )?;
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
                    writeln!(
                        out,
                        "Compiling {} {}",
                        tidy_package_id(package_id),
                        if fresh { "[fresh]" } else { "" }
                    )?;
                }
            }
            "compiler-message" => {
                if let Some(Value::Object(msg)) = event.get("message") {
                    if let Ok(true) = parse_compiler_message(ctx, msg, out, err) {
                        inspection_logged = true;
                    }
                }
            }
            "build-finished" => {}
            _ => {
                writeln!(out, "{}", compiler_msg)?;
                writeln!(out, "{:?}", event)?;
            }
        }
    } else if let Some(Value::String(ttype)) = event.get("type") {
        match ttype.as_ref() {
            "suite" => match event.get("event") {
                Some(Value::String(event_name)) => match event_name.as_ref() {
                    "started" => {
                        writeln!(out, "##{}[testSuiteStarted name='rust_test_suite' flowId='test_suite_flow_id']", brand)?;
                    }
                    "ok" => {
                        writeln!(out, "##{}[testSuiteFinished name='rust_test_suite' flowId='test_suite_flow_id']", brand)?;
                    }
                    "failed" => {
                        inspection_logged = true;
                        writeln!(out, "##{}[testSuiteFinished name='rust_test_suite' flowId='test_suite_flow_id']", brand)?;
                    }
                    _ => {
                        writeln!(out, "format unknown {:?}", event)?;
                    }
                },
                _ => {
                    writeln!(out, "format {:?}", event)?;
                }
            },
            "bench" => {
                let name = parse_name(&event);

                if let Some(Value::Number(median)) = event.get("median") {
                    writeln!(
                        out,
                        "##{}[buildStatisticValue key='bench.{}.median' value='{:.6}']",
                        brand,
                        name,
                        median.as_f64().unwrap()
                    )?;
                }
                if let Some(Value::Number(devation)) = event.get("deviation") {
                    writeln!(
                        out,
                        "##{}[buildStatisticValue key='bench.{}.deviation' value='{:.6}']",
                        brand, name, devation
                    )?;
                }
            }
            "test" => match event.get("event") {
                Some(Value::String(s)) => {
                    return parse_test_event(ctx, s, event, out, err);
                }
                _ => {
                    writeln!(out, "unhandled event - please report: {:?}", event)?;
                }
            },
            _ => {
                writeln!(out, "unhandled event - please report: {:?}", event)?;
            }
        }
    }
    Ok(inspection_logged)
}

fn parse_compiler_message(
    ctx: &Context,
    msg: &Map<String, Value>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<bool, Box<dyn Error>> {
    if let Some(Value::String(level)) = msg.get("level") {
        let level = if level.as_str() == "error: internal compiler error" {
            "error".to_string()
        } else {
            level.to_string()
        };
        if level.as_str() == "warning" || level.as_str() == "error" {
            let message = if let Some(Value::String(message)) = msg.get("rendered") {
                message
            } else {
                ""
            };

            //TODO ask jetbrains if there's a way we can embed html here as message could
            // do with being monospaced.
            // let message = "<pre>".to_string() + &message + "</pre>";

            let (code, explanation) = if let Some(Value::Object(code)) = msg.get("code") {
                let explanation = if let Some(Value::String(explanation)) = code.get("explanation")
                {
                    explanation
                } else {
                    "no explanation"
                };

                if let Some(Value::String(code)) = code.get("code") {
                    (code.as_ref(), explanation)
                } else {
                    ("other", explanation)
                }
            } else {
                ("other", "no explanation")
            };

            let mut file = "no_file";
            let mut line = 0u64;
            if let Some(Value::Array(spans)) = msg.get("spans") {
                if !spans.is_empty() {
                    if let Value::Object(span) = &spans[0] {
                        if let Some(Value::String(file_name)) = span.get("file_name") {
                            file = &file_name;
                        }
                        if let Some(Value::Number(line_number)) = span.get("line_start") {
                            line = line_number.as_u64().unwrap_or(0);
                        }
                    }
                }
            }

            if level == "error" {
                writeln!(
                    out,
                    "##{}[buildProblem description='{}' identity='{}']",
                    ctx.brand,
                    escape_message(message),
                    code
                )?;
                writeln!(err, "{}", message)?;
            } else {
                if !message.contains("1 warning emitted") && !message.contains(" warnings emitted")
                {
                    writeln!(
                        out,
                        "##{}[inspectionType id='{}' category='{}' name='{}' description='{}']",
                        ctx.brand, code, level, code, explanation
                    )?;
                    writeln!(out,
                        "##{}[inspection typeId='{}' message='{}' file='{}' line='{}' SEVERITY='{}']",
                        ctx.brand,
                        code,
                        escape_message(message),
                        file,
                        line,
                        level
                    )?;
                }
                writeln!(out, "{}", message)?;
            }

            //additional attribute='<additional attribute>'
            Ok(true)
        // } else {
        //     println!("unhandled msg: {:?}", msg);
        //     Ok(false)
        // }
        } else {
            writeln!(out, "unhandled message: {:?}", msg)?;
            Ok(false)
        }
    } else {
        writeln!(out, "{:?}", msg)?;
        Ok(false)
    }
}

fn parse_test_event(
    ctx: &Context,
    event_type: &str,
    event: &Map<String, Value>,
    out: &mut dyn Write,
    _err: &mut dyn Write,
) -> Result<bool, Box<dyn Error>> {
    //TODO split parsing from output!
    let name = parse_name(&event);

    match event_type {
        "started" => {
            writeln!(
                out,
                "##{}[flowStarted flowId='{}' parent='test_suite_flow_id']",
                ctx.brand, name
            )?;
            writeln!(out, "##{}[testStarted flowId='{}' name='{}' captureStandardOutput='true' parent='test_suite_flow_id']", ctx.brand, name, name)?;
            Ok(false)
        }
        "ok" => {
            if let Some(Value::String(exec_time)) = event.get("exec_time") {
                writeln!(
                    out,
                    "##{}[testFinished flowId='{}' name='{}' duration='{}']",
                    ctx.brand, name, name, exec_time
                )?;
            } else {
                writeln!(
                    out,
                    "##{}[testFinished flowId='{}' name='{}']",
                    ctx.brand, name, name
                )?;
            }
            writeln!(out, "##{}[flowFinished flowId='{}']", ctx.brand, name)?;
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
                writeln!(out, "##{}[testFailed type='comparisonFailure' name='{}' flowId='{}' message='test failed' details='{}' expected='{}' actual='{}']", ctx.brand, name, name, escape_message(stdout),
                escape_message(left),escape_message(right))?;
            } else {
                writeln!(
                    out,
                    "##{}[testFailed name='{}' flowId='{}' message='test failed' details='{}']",
                    ctx.brand,
                    name,
                    name,
                    escape_message(stdout)
                )?;
            }

            writeln!(
                out,
                "##{}[testFinished flowId='{}' name='{}']",
                ctx.brand, name, name
            )?;
            writeln!(out, "##{}[flowFinished flowId='{}']", ctx.brand, name)?;
            Ok(true)
        }
        _ => {
            writeln!(out, "failed to parse {:?}", event)?;
            Ok(false)
        }
    }
}

fn parse_timing_info(
    ctx: &Context,
    event: &Map<String, Value>,
    out: &mut dyn Write,
    _err: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let name = if let Some(Value::Object(target)) = event.get("target") {
        if let Some(Value::String(target_name)) = target.get("name") {
            target_name
        } else {
            "anon"
        }
    } else {
        "anon"
    };

    let compile_mode = if let Some(Value::String(compile_mode)) = event.get("mode") {
        compile_mode
    } else {
        "mode"
    };

    if let Some(Value::Number(duration)) = event.get("duration") {
        if let Some(duration) = duration.as_f64() {
            if duration > ctx.min_threshold {
                writeln!(
                    out,
                    "##{}[buildStatisticValue key='{} {}' value='{:.6}']",
                    ctx.brand, compile_mode, name, duration
                )?;

                writeln!(out, "Compiled {} in {:.2}s", name, duration)?;
            }
        }
    }
    Ok(())
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
    if let (Some(left_index), Some(right_index)) = (msg.find("left: `"), msg.find("right: `")) {
        if let (Some(left_end), Some(right_end)) = (
            msg[left_index..].find("`,"),
            msg[right_index..].find("`', "),
        ) {
            let left = &msg[(left_index + "left: `".len())..(left_index + left_end)];
            let right = &msg[(right_index + "right: `".len())..(right_index + right_end)];
            return Some((left, right));
        }
    }
    None
}

fn parse_cov(cov: &str) -> (f64, u64, u64, u64) {
    let stream = Deserializer::from_str(&cov);
    for value in stream.into_iter() {
        if let Ok(Value::Object(map)) = value {
            let percent = if let Some(Value::Number(num)) = map.get("coveragePercent") {
                num.as_f64().unwrap_or(0.)
            } else {
                0.
            };
            let lcov = if let Some(Value::Number(num)) = map.get("linesCovered") {
                num.as_i64().unwrap_or(0) as u64
            } else {
                0
            };
            let lmiss = if let Some(Value::Number(num)) = map.get("linesMissed") {
                num.as_i64().unwrap_or(0) as u64
            } else {
                0
            };
            let ltot = if let Some(Value::Number(num)) = map.get("linesTotal") {
                num.as_i64().unwrap_or(0) as u64
            } else {
                0
            };
            return (percent, lcov, lmiss, ltot);
        }
    }
    (0., 0, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    //(Benche isn't stable still!)
    //use test::Bencher;

    //Bench not stable:
    // #[bench]
    // fn example_bench_add_two(b: &mut Bencher) {
    //     b.iter(|| {
    //         print!("hi");
    //     });
    // }

    #[test]
    fn test() {
        assert!(cargo_service_message(vec!["path/to/bin".into()]).is_err());
        assert!(cargo_service_message(vec!["path/to/bin".into(), "fred".to_string()]).is_err());
        //assert_eq!(Ok(()), cargo_service_message(vec!["path/to/bin".into(),"test".to_string()]));
    }

    // #[test]
    // fn test_a_failure_fails() {
    //     assert_eq!("red", "green");
    // }

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

    #[test]
    fn parse_coverage() {
        let cov = r#"
        {
            "children": {
            },
            "coveragePercent": 8.86,
            "linesCovered": 1067,
            "linesMissed": 10975,
            "linesTotal": 12042,
            "name": ""
        }
        "#;
        let (percent, lcov, lmiss, ltot) = parse_cov(cov);
        assert_eq!(percent, 8.86);
        assert_eq!(lcov, 1067);
        assert_eq!(lmiss, 10975);
        assert_eq!(ltot, 12042);
    }

    fn check(line: &str) -> (String, String) {
        let mut out = vec![];
        let mut err = vec![];
        let stream = Deserializer::from_str(&line);
        if let Value::Object(event) = stream.into_iter().next().unwrap().unwrap() {
            let ctx = Context {
                debug: false,
                brand: "t".to_string(),
                min_threshold: 5.,
            };

            process(&ctx, &event, &mut out, &mut err).unwrap();
        } else {
            assert!(false);
        }

        let out = String::from_utf8(out).unwrap().trim_end().to_string();
        let err = String::from_utf8(err).unwrap().trim_end().to_string();
        (out, err)
    }

    #[test]
    fn test_testsuite_start() {
        assert_eq!(
            check(r#"{ "type": "suite", "event": "started", "test_count": 3 }"#),
            (
                "##t[testSuiteStarted name='rust_test_suite' flowId='test_suite_flow_id']".into(),
                "".into()
            )
        );
    }

    #[test]
    fn test_testsuite_ok() {
        assert_eq!(
            check(
                r#"{ "type": "suite", "event": "ok", "passed": 3, "failed": 0, "allowed_fail": 0, "ignored": 0, "measured": 0, "filtered_out": 0 }"#
            ),
            (
                "##t[testSuiteFinished name='rust_test_suite' flowId='test_suite_flow_id']".into(),
                "".into()
            )
        );
    }

    #[test]
    fn test_test_started() {
        assert_eq!(
            check(r#"{ "type": "test", "event": "started", "name": "tests::test" }"#),
            (
                r#"##t[flowStarted flowId='tests.test' parent='test_suite_flow_id']
##t[testStarted flowId='tests.test' name='tests.test' captureStandardOutput='true' parent='test_suite_flow_id']"#.into(),
                "".into()
            )
        );
    }

    #[test]
    fn test_test_success() {
        assert_eq!(
            check(r#"{ "type": "test", "event": "ok", "name": "tests::test_slow", "exec_time": "10.000s" }"#),
            (
                r#"##t[testFinished flowId='tests.test_slow' name='tests.test_slow' duration='10.000s']
##t[flowFinished flowId='tests.test_slow']"#.into(),
                "".into()
            )
        );
    }

    #[test]
    fn test_test_bench() {
        assert_eq!(
            check(
                r#"{ "type": "bench", "name": "tests::example_bench_add_two", "median": 57, "deviation": 9 }"#
            ),
            (r#"##t[buildStatisticValue key='bench.tests.example_bench_add_two.median' value='57.000000']
##t[buildStatisticValue key='bench.tests.example_bench_add_two.deviation' value='9']"#.into(), "".into())
        );
    }

    #[test]
    fn test_ignored_test_does_nothing() {
        //TODO: should we not signal there's an ignored test here?
        assert_eq!(
            check(r#"{"event": "ignored", "name": "tests::test_a_failure_fails", "type": "test"}"#),
            (r#""#.into(), "".into())
        );
    }

    #[test]
    fn test_compiler_message_built_crate() {
        assert_eq!(
            check(
                r#"{"reason":"compiler-artifact","package_id":"cfg-if 0.1.10 (registry+https://github.com/rust-lang/crates.io-index)","target":{"kind":["lib"],"crate_types":["lib"],"name":"cfg-if","src_path":"/Users/gilescope/.cargo/registry/src/github.com-1ecc6299db9ec823/cfg-if-0.1.10/src/lib.rs","edition":"2018","doctest":true,"test":true},"profile":{"opt_level":"0","debuginfo":0,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/Users/gilescope/projects/rust-analyzer/target/debug/deps/libcfg_if-f4d43ad0e841b013.rlib","/Users/gilescope/projects/rust-analyzer/target/debug/deps/libcfg_if-f4d43ad0e841b013.rmeta"],"executable":null,"fresh":false}"#
            ),
            (r#"Compiling cfg-if 0.1.10"#.into(), "".into())
        );
    }

    #[test]
    fn test_compiler_message_timing_info_do_nothing_below_threshold() {
        assert_eq!(
            check(
                r#"{"reason":"timing-info","package_id":"cfg-if 0.1.10 (registry+https://github.com/rust-lang/crates.io-index)","target":{"kind":["lib"],"crate_types":["lib"],"name":"cfg-if","src_path":"/Users/gilescope/.cargo/registry/src/github.com-1ecc6299db9ec823/cfg-if-0.1.10/src/lib.rs","edition":"2018","doctest":true,"test":true},"mode":"build","duration":0.23798263500000005,"rmeta_time":0.20777650800000003}"#
            ),
            (r#""#.into(), "".into())
        );
    }

    #[test]
    fn test_compiler_message_timing_info_add_stats_when_above_threshold() {
        assert_eq!(
            check(
                r#"{"reason":"timing-info","package_id":"cfg-if 0.1.10 (registry+https://github.com/rust-lang/crates.io-index)","target":{"kind":["lib"],"crate_types":["lib"],"name":"cfg-if","src_path":"/Users/gilescope/.cargo/registry/src/github.com-1ecc6299db9ec823/cfg-if-0.1.10/src/lib.rs","edition":"2018","doctest":true,"test":true},"mode":"build","duration":20.2379826350000005,"rmeta_time":20.2077765080000003}"#
            ),
            (
                r#"##t[buildStatisticValue key='build cfg-if' value='20.237983']
Compiled cfg-if in 20.24s"#
                    .into(),
                "".into()
            )
        );
    }

    #[test]
    fn test_compiler_message_build_finished() {
        assert_eq!(
            check(r#"{"reason":"build-finished","success":false}"#),
            (r#""#.into(), "".into())
        );
    }

    #[test]
    fn test_compiler_message_unused_var_warning() {
        assert_eq!(
            check(
                r#"{"reason":"compiler-message","package_id":"cargo-service-message 0.1.4 (path+file:///Users/gilescope/projects/cargo-service-message2)","target":{"kind":["bin"],"crate_types":["bin"],"name":"cargo-service-message","src_path":"/Users/gilescope/projects/cargo-service-message2/src/bin/cargo-service-message.rs","edition":"2018","doctest":false,"test":true},"message":{"rendered":"warning: unused variable: `x`\n   --> src/bin/cargo-service-message.rs:326:9\n    |\n326 |     let x = \"\";\n    |         ^ help: if this is intentional, prefix it with an underscore: `_x`\n    |","children":[{"children":[],"code":null,"level":"note","message":"`#[warn(unused_variables)]` on by default","rendered":null,"spans":[]},{"children":[],"code":null,"level":"help","message":"if this is intentional, prefix it with an underscore","rendered":null,"spans":[{"byte_end":10932,"byte_start":10931,"column_end":10,"column_start":9,"expansion":null,"file_name":"src/bin/cargo-service-message.rs","is_primary":true,"label":null,"line_end":326,"line_start":326,"suggested_replacement":"_x","suggestion_applicability":"MachineApplicable","text":[{"highlight_end":10,"highlight_start":9,"text":"    let x = \"\";"}]}]}],"code":{"code":"unused_variables","explanation":null},"level":"warning","message":"unused variable: `x`","spans":[{"byte_end":10932,"byte_start":10931,"column_end":10,"column_start":9,"expansion":null,"file_name":"src/bin/cargo-service-message.rs","is_primary":true,"label":null,"line_end":326,"line_start":326,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"highlight_end":10,"highlight_start":9,"text":"    let x = \"\";"}]}]}}"#
            ),
            (r###"##t[inspectionType id='unused_variables' category='warning' name='unused_variables' description='no explanation']
##t[inspection typeId='unused_variables' message='warning: unused variable: `x`|n   --> src/bin/cargo-service-message.rs:326:9|n    |||n326 ||     let x = "";|n    ||         ^ help: if this is intentional, prefix it with an underscore: `_x`|n    ||' file='src/bin/cargo-service-message.rs' line='326' SEVERITY='warning']
warning: unused variable: `x`
   --> src/bin/cargo-service-message.rs:326:9
    |
326 |     let x = "";
    |         ^ help: if this is intentional, prefix it with an underscore: `_x`
    |"###.into(), "".into()) 
        );
    }

    #[test]
    fn test_compiler_message_error() {
        assert_eq!(
            check(
                r###"{"reason":"compiler-message","package_id":"cargo-service-message 0.1.4 (path+file:///Users/gilescope/projects/cargo-service-message2)","target":{"kind":["bin"],"crate_types":["bin"],"name":"cargo-service-message","src_path":"/Users/gilescope/projects/cargo-service-message2/src/bin/cargo-service-message.rs","edition":"2018","doctest":false,"test":true},"message":{"rendered":"error: expected one of `!` or `::`, found `tests`\n   --> src/bin/cargo-service-message.rs:747:6\n    |\n747 | moXd tests {\n    |      ^^^^^ expected one of `!` or `::`\n\n","children":[],"code":null,"level":"error","message":"expected one of `!` or `::`, found `tests`","spans":[{"byte_end":25339,"byte_start":25334,"column_end":11,"column_start":6,"expansion":null,"file_name":"src/bin/cargo-service-message.rs","is_primary":true,"label":"expected one of `!` or `::`","line_end":747,"line_start":747,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"highlight_end":11,"highlight_start":6,"text":"moXd tests {"}]}]}}"###
            ),
            (r###"##t[buildProblem description='error: expected one of `!` or `::`, found `tests`|n   --> src/bin/cargo-service-message.rs:747:6|n    |||n747 || moXd tests {|n    ||      ^^^^^ expected one of `!` or `::`|n|n' identity='other']"###.into(), 
            r##"error: expected one of `!` or `::`, found `tests`
   --> src/bin/cargo-service-message.rs:747:6
    |
747 | moXd tests {
    |      ^^^^^ expected one of `!` or `::`"##.into()) //TODO:
        );
    }
}
