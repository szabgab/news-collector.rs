use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus};

fn compile() {
    let result = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .output()
        .expect("failed to execute process");
    //print!("{}", std::str::from_utf8(&result.stdout).unwrap());
    //print!("{}", std::str::from_utf8(&result.stderr).unwrap());
    //println!("{}", result.status);
    assert!(result.status.success());
}

#[test]
fn missing_config() {
    compile();

    let result = Command::new("./target/release/news-collector")
        .output()
        .expect("failed to execute process");
    let stdout = std::str::from_utf8(&result.stdout).unwrap();
    let stderr = std::str::from_utf8(&result.stderr).unwrap();
    // print!("stdout: {stdout}");
    // print!("stderr: {stderr}");
    // println!("{}", result.status);
    assert!(stdout.contains("Starting the News collector"));
    assert!(stderr.contains("Usage: news-collector --config <CONFIG>"));

    assert!(!result.status.success());
    assert_eq!(result.status, ExitStatus::from_raw(256 * 2));
}

#[test]
fn bad_config_file() {
    compile();

    let result = Command::new("./target/release/news-collector")
        .arg("--config")
        .arg("qqrq.yaml")
        .output()
        .expect("failed to execute process");

    let stdout = std::str::from_utf8(&result.stdout).unwrap();
    let stderr = std::str::from_utf8(&result.stderr).unwrap();
    //println!("stdout: '{stdout}'");
    //println!("stderr: '{stderr}'");
    assert!(stdout.contains("Starting the News collector"));
    assert!(stdout.contains("ERROR [news_collector] Config file 'qqrq.yaml' could not be read No such file or directory"));
    assert!(stderr.is_empty());
    // println!("{}", result.status);

    assert_eq!(result.status, ExitStatus::from_raw(256 * 1));
}

#[test]
fn read_config_file() {
    compile();

    let result = Command::new("./target/release/news-collector")
        .arg("--config")
        .arg("dev.yaml")
        .output()
        .expect("failed to execute process");

    let stdout = std::str::from_utf8(&result.stdout).unwrap();
    let stderr = std::str::from_utf8(&result.stderr).unwrap();
    //print!("stdout: {stdout}");
    //print!("stderr: {stderr}");
    assert!(stdout.contains("Starting the News collector"));
    assert!(stderr.is_empty());
    // println!("{}", result.status);

    assert_eq!(result.status, ExitStatus::from_raw(0));
}
