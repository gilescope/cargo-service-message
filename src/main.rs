#[macro_use]
extern crate serde_derive;

use std::process::{Command, Stdio};
use serde_json::{Deserializer};
use std::error::Error;

fn main() -> Result<(), String> {
    let options : Vec<String> = std::env::args().collect();
    println!("{:?}", &options);
    cargo_service_message(options)
}

fn cargo_service_message(argv: Vec<String>) -> Result<(), String> {
    if argv.len() < 2  {
        return Err(format!("Usage: 'test' as the next argument followed by the standard cargo test arguments. Found {:?}", argv));//eyre!("swoops"));
    }
    if argv[1] != "service-message".to_string() {
        return Err(format!("expected 'service-message' as the next argument followed by the standard cargo test arguments but got {}", argv[1]));//eyre!("swoops"));
    }
    if argv[2] != "test".to_string() {
        return Err(format!("expected 'test' as the next argument followed by the standard cargo test arguments but got {}", argv[2]));//eyre!("swoops"));
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

#[derive(Deserialize, Debug)]
#[serde(tag="type")]
enum Event {
    #[serde(alias = "suite")]
    Suite {
        event: String,
        test_count: Option<i32>,
        passed: Option<i32>,
        failed: Option<i32>,
        allowed_fail: Option<i32>,
        ignored: Option<i32>,
        measured: Option<i32>,
        filtered_out: Option<i32>
    },
    #[serde(alias = "test")]
    Test {
        event: String,
        name: String,
        exec_time: Option<String>,
        stdout: Option<String>
    }
}

fn run_tests(args: &[String]) -> Result<(), Box<dyn Error>> {
    println!("running");
    let mut cmd = Command::new("cargo");
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.args(args);

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
    let stream = Deserializer::from_reader(child.stdout.unwrap()).into_iter::<Event>();

    for value in stream {
        match value {
            Ok(Event::Suite{
                event,
                ..
            }) => {
                match event.as_str() {
                    "started" => {
                    println!("##{}[testSuiteStarted name='{}' flowId='{}']", brand, "a_test_suite_name", "test_suite_flow_id");
                    }
                    "ok" => {
                        println!("##{}[testSuiteFinished name='{}' flowId='{}']", brand, "a_test_suite_name", "test_suite_flow_id");
                    }
                    _ =>{ println!("format {}", event);}
                }
            },
            Ok(Event::Test {
                event,
                name,
                exec_time,
                stdout,
            }) => {
                let name = name.replace("::", ".");
                match event.as_str() {
                    "started" => {
                    println!("##{}[testStarted name='{}' flowId='{}' parent='test_suite_flow_id']", brand, name, name);
                    }
                    "ok" => {
                        if let Some(exec_time) = exec_time {
                            println!("##{}[testFinished name='{}' flowId='{}' duration='{}']", brand, name, name, exec_time);
                        } else {
                            println!("##{}[testFinished name='{}' flowId='{}']", brand, name, name);
                        }
                    }
                    "failed" => {
                        println!("##{}[testFailed name='{}' flowId='{}' message='test failed' details='{}']", brand, name, name, escape_message(stdout.unwrap_or_else(String::new)));
                        //special support for comparison failures expected / actual.
                        //##teamcity[testFailed type='comparisonFailure' name='MyTest.test2' message='failure message' details='message and stack trace' expected='expected value' actual='actual value']
                    }
                    _ =>{ println!("format {}", event);}
                }
            },
            Err(err) => {
                println!("error {}", err);
            }
        }
    }
    println!("fin");
    Ok(())
}

fn escape_message(unescaped: String) -> String {
    //TODO:\uNNNN (unicode symbol with code 0xNNNN)  as |0xNNNN
    unescaped.replace("|", "||").replace("[", "|[").replace("]", "|]").replace("\n", "|n").replace("\r", "|r").replace("'", "|'")
}

fn contains(needle: &str, args: &[String]) -> bool
{
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
    fn test_fast() {
        assert_eq!("red", "green");
    }

    #[test]
    fn test_slow() {
        std::thread::sleep(std::time::Duration::new(1,0));
    }
}