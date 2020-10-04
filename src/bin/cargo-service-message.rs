use serde_json::{Deserializer, Value};
use std::error::Error;
use std::process::{Command, Stdio};

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
    if argv[2] != *"test" {
        return Err(format!("expected 'test' as the next argument followed by the standard cargo test arguments but got {}", argv[2]));
        //eyre!("swoops"));
    }

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
    println!("running");
    let mut cmd = Command::new("cargo");
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.args(args);

    cmd.arg("--message-format=json"); //TODO this needs to be before --

    if !contains("--", args) {
        cmd.arg("--");
    }
    if !contains("-Zunstable-options", args) && !contains("unstable-options", args) {
        cmd.arg("-Zunstable-options");
    }
    cmd.arg("--format");
    cmd.arg("json");
    println!("spawning: {:?}", &cmd);
    let child = cmd.spawn()?;

    let brand = "teamcity";
    let stream = Deserializer::from_reader(child.stdout.unwrap()).into_iter::<Value>();

    for value in stream {
        match value {
            Ok(Value::Object(event)) => {
                if let Some(Value::String(ttype)) = event.get("type") {
                    match ttype.as_ref() {
                        "suite" => match event.get("event") {
                            Some(Value::String(event_name)) => match event_name.as_ref() {
                                "started" => {
                                    println!(
                                        "##{}[testSuiteStarted name='{}' flowId='{}']",
                                        brand, "a_test_suite_name", "test_suite_flow_id"
                                    );
                                }
                                "ok" => {
                                    println!(
                                        "##{}[testSuiteFinished name='{}' flowId='{}']",
                                        brand, "a_test_suite_name", "test_suite_flow_id"
                                    );
                                }
                                "failed" => {
                                    println!(
                                        "##{}[testSuiteFinished name='{}' flowId='{}']",
                                        brand, "a_test_suite_name", "test_suite_flow_id"
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
                                            println!("##{}[testFailed name='{}' flowId='{}' message='test failed' details='{}']", brand, name, name, escape_message(stdout.to_string()));
                                            println!("##{}[testFinished flowId='{}' name='{}']", brand, name, name);
                                            //special support for comparison failures expected / actual.
                                            //##teamcity[testFailed type='comparisonFailure' name='MyTest.test2' message='failure message' details='message and stack trace' expected='expected value' actual='actual value']
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
}
