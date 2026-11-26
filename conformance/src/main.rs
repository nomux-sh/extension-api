//! Runs the conformance suite against an extension program.
//!
//! ```text
//! nomux-conformance --manifest <extension.toml> [options] -- <program> [arguments]
//! ```

use std::ffi::OsString;
use std::process::ExitCode;
use std::{env, fs, panic};

use nomux_conformance::{CASES, Target, measure_startup};
use nomux_extension::manifest::Manifest;

const USAGE: &str = "\
usage: nomux-conformance --manifest <extension.toml> [options] -- <program> [arguments]

options:
  --case <name>             run only this case; repeat it for more cases
  --list                    print the cases and exit
  --show-stderr             show the standard error of the extension
  --measure-startup <runs>  print the time from the process start to Welcome";

struct Options {
    manifest: Option<OsString>,
    cases: Vec<String>,
    list: bool,
    show_stderr: bool,
    startup_runs: Option<usize>,
    command: Vec<OsString>,
}

fn main() -> ExitCode {
    match parse(env::args_os().skip(1).collect()) {
        Ok(options) => run(options),
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn parse(arguments: Vec<OsString>) -> Result<Options, String> {
    let mut options = Options {
        manifest: None,
        cases: Vec::new(),
        list: false,
        show_stderr: false,
        startup_runs: None,
        command: Vec::new(),
    };
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.to_str() {
            Some("--manifest") => options.manifest = Some(value("--manifest")?),
            Some("--case") => options.cases.push(text(value("--case")?)?),
            Some("--list") => options.list = true,
            Some("--show-stderr") => options.show_stderr = true,
            Some("--measure-startup") => {
                let runs = text(value("--measure-startup")?)?;
                let runs = runs
                    .parse()
                    .map_err(|_| format!("--measure-startup needs a number, not `{runs}`"))?;
                options.startup_runs = Some(runs);
            }
            Some("--") => {
                options.command = arguments.collect();
                break;
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    Ok(options)
}

fn text(value: OsString) -> Result<String, String> {
    value
        .into_string()
        .map_err(|value| format!("`{}` is not UTF-8", value.display()))
}

fn run(options: Options) -> ExitCode {
    if options.list {
        for case in CASES {
            println!("{:<18} {}", case.name, case.summary);
        }
        return ExitCode::SUCCESS;
    }
    let target = match target(&options) {
        Ok(target) => target,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if let Some(runs) = options.startup_runs {
        let times = measure_startup(&target, runs);
        println!(
            "process start to Welcome over {runs} runs: median {:.1} ms, minimum {:.1} ms",
            times.median.as_secs_f64() * 1000.0,
            times.minimum.as_secs_f64() * 1000.0
        );
        return ExitCode::SUCCESS;
    }
    run_cases(&target, &options.cases)
}

fn target(options: &Options) -> Result<Target, String> {
    if options.command.is_empty() {
        return Err("the program of the extension is missing after `--`".to_owned());
    }
    let path = options.manifest.as_ref().ok_or("--manifest is missing")?;
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{} was not read: {error}", path.display()))?;
    let manifest: Manifest = text
        .parse()
        .map_err(|error| format!("{} is not valid: {error}", path.display()))?;
    for name in &options.cases {
        if !CASES.iter().any(|case| case.name == name) {
            return Err(format!(
                "there is no case `{name}`; --list prints the cases"
            ));
        }
    }
    Ok(Target {
        command: options.command.clone(),
        manifest,
        show_stderr: options.show_stderr,
    })
}

fn run_cases(target: &Target, names: &[String]) -> ExitCode {
    // A failed case is a panic. Its message is in the report.
    panic::set_hook(Box::new(|_| {}));
    let mut failures = 0;
    let selected = CASES
        .iter()
        .filter(|case| names.is_empty() || names.iter().any(|name| name == case.name));
    for case in selected {
        match case.run(target) {
            Ok(()) => println!("ok    {}", case.name),
            Err(reason) => {
                failures += 1;
                println!("FAIL  {}: {reason}", case.name);
            }
        }
    }
    if failures == 0 {
        println!("all cases passed");
        ExitCode::SUCCESS
    } else {
        println!("{failures} case(s) failed");
        ExitCode::FAILURE
    }
}
