//! The program the tests run: it does what its arguments say, one after the other.
//! `out N`, `err N` and `both N` print lines; `sleep MS` waits; `grandchild MS` starts itself
//! to sleep that long and waits for it; `echo` repeats each line of its input until it closes;
//! `env NAME` prints the variable or `<none>`; `long N` prints a line of N bytes; `fail CODE`
//! ends with that code.
#![allow(clippy::expect_used, reason = "a program for tests, which fails loudly")]

use std::io::{BufRead, Write};
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    let number = |i: usize| args.get(i + 1).and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
    while i < args.len() {
        match args[i].as_str() {
            "out" => (0..number(i)).for_each(|n| println!("out {n}")),
            "err" => (0..number(i)).for_each(|n| eprintln!("err {n}")),
            "both" => (0..number(i)).for_each(|n| {
                println!("out {n}");
                eprintln!("err {n}");
            }),
            "sleep" => std::thread::sleep(Duration::from_millis(number(i))),
            "grandchild" => {
                let me = std::env::current_exe().expect("itself");
                let mut child = std::process::Command::new(me)
                    .args(["sleep", &number(i).to_string()])
                    .spawn()
                    .expect("a grandchild");
                println!("grandchild {}", child.id());
                let _ = std::io::stdout().flush();
                let _ = child.wait();
            }
            "echo" => {
                for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                    println!("echo {line}");
                    let _ = std::io::stdout().flush();
                }
                i += 1;
                continue;
            }
            "env" => println!(
                "{}",
                std::env::var(args.get(i + 1).map_or("", String::as_str)).unwrap_or("<none>".into())
            ),
            "long" => println!("{}", "x".repeat(number(i) as usize)),
            "fail" => std::process::exit(number(i) as i32),
            _ => {}
        }
        let _ = std::io::stdout().flush();
        i += if matches!(args[i].as_str(), "echo") { 1 } else { 2 };
    }
}
