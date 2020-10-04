use serde_json::{Deserializer, Value};
use std::error::Error;
use std::process::{Command, Stdio};
use std::collections::HashSet;

fn main() -> Result<(), String> {
    let options: Vec<String> = std::env::args().collect();
    println!("{:?}", &options);
    cargo_service_message(options)
}

fn cargo_service_message(argv: Vec<String>) -> Result<(), String> {
    if argv.len() < 2 {
        return Err(format!("Usage: 'test' as the next argument followed by the standard cargo test arguments. Found {:?}", argv));
        //eyre!("swoops"));
    }
    if argv[1] != *"service-message" {
        return Err(format!("expected 'service-message' as the next argument followed by the standard cargo test arguments but got {}", argv[1]));
        //eyre!("swoops"));
    }
    // if argv[2] != *"test" {
    //     return Err(format!("expected 'test' as the next argument followed by the standard cargo test arguments but got {}", argv[2]));
    //     //eyre!("swoops"));
    // }

    run_tests(&argv[2..]).unwrap();
    Ok(())
}

/*
{ "type": "test", "event": "started", "name": "tests::test" }
{ "type": "test", "event": "started", "name": "tests::test_fast" }
{ "type": "test", "event": "started", "name": "tests::test_slow" }

{ "type": "test", "event": "ok", "name": "tests::test", "exec_time": "0.000s" }
{ "type": "test", "event": "ok", name": "tests::test_fast", "exec_time": "0.000s" }
{ "type": "test", "event": "ok", "name": "tests::test_slow", "exec_time": "10.000s" }

{ "type": "suite", "event": "started", "test_count": 3 }
{ "type": "suite", "event": "ok", "passed": 3, "failed": 0, "allowed_fail": 0, "ignored": 0, "measured": 0, "filtered_out": 0 }
*/

fn run_tests(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut dedupe = HashSet::new();

    println!("running");
    let mut cmd = Command::new("cargo");
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.args(args);

    let cargo_cmd = args[0].clone(); //TODO support +nightly

    cmd.arg("--message-format=json"); //TODO this needs to be before --

    if !contains("--", args) {
        cmd.arg("--");
    }

    if cargo_cmd == "test" {
        if !contains("-Zunstable-options", args) && !contains("unstable-options", args) {
            cmd.arg("-Zunstable-options");
        }
        cmd.arg("--format");
        cmd.arg("json");
    }
    println!("spawning: {:?}", &cmd);
    let child = cmd.spawn()?;

    let brand = "teamcity";
    let stream = Deserializer::from_reader(child.stdout.unwrap()).into_iter::<Value>();
    let x = String::new();
    for value in stream {
        match value {
            Ok(Value::Object(event)) => {
                if let Some(Value::String(compiler_msg)) = event.get("reason") {
                    match compiler_msg.as_ref() { 
                        "compiler-message" => {
                            if let Some(Value::Object(msg)) = event.get("message") {
                                match msg.get("level") {
                                    Some(Value::String(level)) => {
                                        if level.as_str() == "warning" {
                                            let message = if let Some(Value::String(message)) = msg.get("rendered") {
                                                
                                                message.to_string()
                                            } else {"".to_string()};

                                            if message.len() > 0 {
                                                // Rust has a habbit of giving you the same error message twice.
                                                // Let's cut that out here.
                                                if dedupe.contains(&message) {
                                                    continue;
                                                }
                                                dedupe.insert(message.clone());
                                            }
                                            //TODO ask jetbrains if there's a way we can embed html here as message could
                                            // do with being monospaced.
//                                            let message = "<pre>".to_string() + &message + "</pre>";

                                            if let Some(Value::Object(code)) = msg.get("code") {
                                                let explanation = if let Some(Value::String(explanation)) = code.get("explanation") {
                                                    explanation.to_string()
                                                } else { "no explanation".to_string() };
                                                
                                                let code = if let Some(Value::String(code)) = code.get("code") {
                                                    code.to_string()
                                                } else { "other".to_string() };
                                                
                                                let mut file = "";
                                                let mut line = 0u64;
                                                if let Some(Value::Array(spans)) = msg.get("spans") {
                                                    if let Value::Object(span) = &spans[0] {
                                                        if let Some(Value::String(file_name)) = span.get("file_name")
                                                        {
                                                            file = &file_name;
                                                        }
                                                        if let Some(Value::Number(line_number)) = span.get("line_start")
                                                        {
                                                            line = line_number.as_u64().unwrap_or(0);
                                                        }
                                                    }
                                                }
                                             
                                                println!("##teamcity[inspectionType id='{}' category='warning' name='{}' description='{}']", code, code, explanation);
                                                println!("##{}[inspection typeId='{}' message='{}' file='{}' line='{}' SEVERITY='{}']", brand, code, escape_message(message), file, line, level);
                                                //additional attribute='<additional attribute>'
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                                println!("{:?}", event);
                            }
                        }
                        _ => {
                            println!("{}", compiler_msg);
                            println!("{:?}", event);
                        }
                    }
                }
                else if let Some(Value::String(ttype)) = event.get("type") {
                    match ttype.as_ref() {
                        "suite" => match event.get("event") {
                            Some(Value::String(event_name)) => match event_name.as_ref() {
                                "started" => {
                                    println!(
                                        "##{}[testSuiteStarted name='{}' flowId='{}']",
                                        brand, "rust_test_suite", "test_suite_flow_id"
                                    );
                                }
                                "ok" => {
                                    println!(
                                        "##{}[testSuiteFinished name='{}' flowId='{}']",
                                        brand, "rust_test_suite", "test_suite_flow_id"
                                    );
                                }
                                "failed" => {
                                    println!(
                                        "##{}[testSuiteFinished name='{}' flowId='{}']",
                                        brand, "rust_test_suite", "test_suite_flow_id"
                                    );
                                }
                                _ => {
                                    println!("format unknown {:?}", event);
                                }
                            },
                            _ => {
                                println!("format {:?}", event);
                            }
                        },
                        "test" => {
                            let name = if let Value::String(name) = event.get("name").unwrap() {
                                name
                            } else {
                                "no_name"
                            };
                            let name = name.replace("::", ".").to_string();

                            match event.get("event") {
                                Some(Value::String(s)) => {
                                    match s.as_ref() {
                                        "started" => {
                                            println!("##{}[flowStarted flowId='{}' parent='test_suite_flow_id']", brand, name);
                                            println!("##{}[testStarted flowId='{}' name='{}' captureStandardOutput='true' parent='test_suite_flow_id']", brand, name, name);
                                        }
                                        "ok" => {
                                            if let Some(exec_time) = event.get("exec_time") {
                                                println!("##{}[testFinished flowId='{}' name='{}' duration='{}']", brand, name, name, exec_time);
                                            } else {
                                                println!(
                                                    "##{}[testFinished flowId='{}' name='{}']",
                                                    brand, name, name
                                                );
                                            }
                                            println!("##{}[flowFinished flowId='{}']", brand, name);
                                        }
                                        "failed" => {
                                            let stdout = if let Value::String(stdout) =
                                                event.get("stdout").unwrap()
                                            {
                                                stdout
                                            } else {
                                                ""
                                            };
                                            if let Some((left, right)) = find_comparison(stdout) {
                                                println!("##{}[testFailed type='comparisonFailure' name='{}' flowId='{}' message='test failed' details='{}' expected='{}' actual='{}']", brand, name, name, escape_message(stdout.to_string()), 
                                                escape_message(left.to_string()),escape_message(right.to_string()));
                                            } else {
                                                println!("##{}[testFailed name='{}' flowId='{}' message='test failed' details='{}']", brand, name, name, escape_message(stdout.to_string()));
                                            }

                                            println!("##{}[testFinished flowId='{}' name='{}']", brand, name, name);
                                            //special support for comparison failures expected / actual.
//                                            find_comparison
                                            //##teamcity[testFailed t name='ck trace' expected='expected value' actual='actual value']
                                            println!("##{}[flowFinished flowId='{}']", brand, name);
                                        }
                                        _ => {
                                            println!("failed to parse {:?}", event);
                                        }
                                    }
                                }
                                _ => {
                                    println!("format {:?}", event);
                                }
                            }
                        }
                        _ => {
                            println!("format {:?}", event);
                        }
                    }
                }
            }
            Ok(_) => {
                println!("error parsing cargo output");
            }
            Err(err) => {
                println!("error parsing cargo output: {} (continuing)", err);
            }
        }
    }
    println!("fin");
    Ok(())
}

fn escape_message(unescaped: String) -> String {
    //TODO:\uNNNN (unicode symbol with code 0xNNNN)  as |0xNNNN
    unescaped
        .replace("|", "||")
        .replace("[", "|[")
        .replace("]", "|]")
        .replace("\n", "|n")
        .replace("\r", "|r")
        .replace("'", "|'")
}

fn contains(needle: &str, args: &[String]) -> bool {
    args.iter().any(|x| x == needle)
}

fn find_comparison<'msg>(msg: &'msg str) -> Option<(&'msg str, &'msg str)> {
    if let Some(index) = msg.find("left: `") {
        if let Some(index_end) = msg[index..].find("`,") {
            let left_end = index + index_end;
            let left = &msg[(index + "left: `".len())..left_end];

            if let Some(right_index) = msg.find("right: `") {
                if let Some(right_end) = msg[right_index..].find("`', ") {
                    let right = &msg[(right_index + "right: `".len())..(right_index + right_end)];
                    return Some((left,right));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
