use std::env;
use std::path::Path;
use std::process::{Command, ExitCode};

const LAST_DAY: u8 = 6;

#[derive(Clone, Copy)]
struct TestCommand {
    package: &'static str,
    target_kind: &'static str,
    target: &'static str,
    filter: Option<&'static str>,
}

const DAY_01: &[TestCommand] = &[
    integration("vector-core-starter", "indexes", "day_01_"),
    integration("vector-datafusion-starter", "sql", "day_01_"),
    integration("vector-datafusion-starter", "sqllogictest", "day_01_"),
];

const DAY_02: &[TestCommand] = &[
    integration("vector-core-starter", "indexes", "day_02_"),
    integration("vector-datafusion-starter", "sqllogictest", "day_02_"),
];

const DAY_03: &[TestCommand] = &[
    unit("vector-core-starter", "day_03_"),
    integration("vector-core-starter", "indexes", "day_03_"),
    integration("vector-datafusion-starter", "sqllogictest", "day_03_"),
];

const DAY_04: &[TestCommand] = &[
    unit("vector-core-starter", "day_04_"),
    integration("vector-core-starter", "indexes", "day_04_"),
    integration("vector-datafusion-starter", "sql", "day_04_"),
    integration("vector-datafusion-starter", "sqllogictest", "day_04_"),
];

const DAY_05: &[TestCommand] = &[
    integration("vector-core-starter", "indexes", "day_05_"),
    integration("vector-datafusion-starter", "sql", "day_05_"),
    integration("vector-datafusion-starter", "sqllogictest", "day_05_"),
];

const DAY_06: &[TestCommand] = &[
    all_tests("vector-benchmark-support"),
    integration("vector-core-starter", "indexes", "day_06_"),
    example("vector-core-starter", "recall", "day_06_"),
    integration("vector-core-starter", "sift_smoke", "day_06_"),
];

const fn integration(
    package: &'static str,
    target: &'static str,
    filter: &'static str,
) -> TestCommand {
    TestCommand {
        package,
        target_kind: "--test",
        target,
        filter: Some(filter),
    }
}

const fn example(package: &'static str, target: &'static str, filter: &'static str) -> TestCommand {
    TestCommand {
        package,
        target_kind: "--example",
        target,
        filter: Some(filter),
    }
}

const fn all_tests(package: &'static str) -> TestCommand {
    TestCommand {
        package,
        target_kind: "--lib",
        target: "",
        filter: None,
    }
}

const fn unit(package: &'static str, filter: &'static str) -> TestCommand {
    TestCommand {
        package,
        target_kind: "--lib",
        target: "",
        filter: Some(filter),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let day = parse_day(args.next())?;
    if args.next().is_some() {
        return Err(usage());
    }

    match command.as_str() {
        "test-day" => run_day(day),
        "test-through" => {
            for current in 1..=day {
                run_day(current)?;
            }
            Ok(())
        }
        _ => Err(usage()),
    }
}

fn parse_day(value: Option<String>) -> Result<u8, String> {
    let value = value.ok_or_else(usage)?;
    let day = value
        .parse::<u8>()
        .map_err(|_| format!("day must be an integer from 1 through {LAST_DAY}"))?;
    if !(1..=LAST_DAY).contains(&day) {
        return Err(format!("day must be an integer from 1 through {LAST_DAY}"));
    }
    Ok(day)
}

fn run_day(day: u8) -> Result<(), String> {
    println!("== Rust learner day {day:02} ==");
    for test in commands(day) {
        let mut command = Command::new("cargo");
        command.current_dir(workspace_root()).args([
            "test",
            "--locked",
            "-p",
            test.package,
            test.target_kind,
        ]);
        if !test.target.is_empty() {
            command.arg(test.target);
        }
        if let Some(filter) = test.filter {
            command.arg(filter);
        }
        println!("+ {command:?}");
        let status = command
            .status()
            .map_err(|error| format!("failed to start cargo for day {day}: {error}"))?;
        if !status.success() {
            return Err(format!("Rust learner day {day:02} failed"));
        }
    }
    Ok(())
}

fn commands(day: u8) -> &'static [TestCommand] {
    match day {
        1 => DAY_01,
        2 => DAY_02,
        3 => DAY_03,
        4 => DAY_04,
        5 => DAY_05,
        6 => DAY_06,
        _ => unreachable!("day was validated"),
    }
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live directly under the workspace root")
}

fn usage() -> String {
    "usage: cargo x test-day <1-6> | cargo x test-through <1-6>".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_day_has_a_bounded_test_set() {
        for day in 1..=LAST_DAY {
            assert!(!commands(day).is_empty());
            for test in commands(day) {
                if let Some(filter) = test.filter {
                    assert!(filter.starts_with(&format!("day_{day:02}_")));
                }
            }
        }
    }

    #[test]
    fn cumulative_order_stops_at_the_requested_day() {
        let through_day_four = (1..=4).flat_map(commands).collect::<Vec<_>>();
        assert!(through_day_four.iter().all(|test| {
            test.filter
                .is_none_or(|filter| !filter.starts_with("day_05_"))
        }));
        assert_eq!(through_day_four.len(), 12);
    }
}
