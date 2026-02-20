use std::io::{self, BufRead, Read, Write};

fn main() {
    match std::env::args().nth(1) {
        Some(path) => run_file(&path),
        None => {
            if unsafe { libc::isatty(libc::STDIN_FILENO) } != 0 {
                repl();
            } else {
                run_stdin();
            }
        }
    }
}

fn run_file(path: &str) {
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error reading {}: {}", path, e);
        std::process::exit(1);
    });
    match nana::run_with_warnings(&source) {
        Ok((val, warnings)) => {
            for w in &warnings {
                eprintln!("{}", w);
            }
            println!("{}", val);
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn run_stdin() {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
        eprintln!("error reading stdin: {}", e);
        std::process::exit(1);
    });
    match nana::run_with_warnings(&buf) {
        Ok((val, warnings)) => {
            for w in &warnings {
                eprintln!("{}", w);
            }
            println!("{}", val);
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn repl() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut env = nana::default_env();

    eprintln!("nana repl — type expressions, press enter to evaluate. Ctrl-D to exit.");
    loop {
        eprint!(">> ");
        io::stderr().flush().ok();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,                  // EOF
            Err(e) => {
                eprintln!("read error: {}", e);
                break;
            }
            Ok(_) => {}
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match nana::run_in_env(line, &env) {
            Ok((val, new_env)) => {
                env = new_env;
                println!("{}", val);
            }
            Err(e) => eprintln!("{}", e),
        }
    }
}
