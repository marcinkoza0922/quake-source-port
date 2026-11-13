//! qinspect: dump game data. Currently only `progs` (QuakeC disassembler).

use std::process::ExitCode;

use qcore::fs::Fs;
use qvm::Progs;
use qvm::disasm::Disasm;

const USAGE: &str = "usage: qinspect progs [FILE | -g GAME[,GAME..]] [--func NAME].. [--all] [--grep TEXT]
  default prints a summary (counts, files, builtins used)
  --func NAME  disassemble one function (repeatable)
  --all        disassemble every function
  --grep TEXT  list functions whose listing contains TEXT";

fn load(file: Option<&str>, games: Option<&str>) -> Result<Vec<u8>, String> {
    if let Some(f) = file {
        return std::fs::read(f).map_err(|e| format!("{f}: {e}"));
    }
    let mut fs = Fs::new(Fs::default_basedir());
    fs.add_game_dir("id1");
    for g in games.unwrap_or("").split(',').filter(|g| !g.is_empty()) {
        if !fs.add_game_dir(g.trim_start_matches('+')) {
            return Err(format!("no game dir {g}"));
        }
    }
    fs.open("progs.dat").ok_or_else(|| "progs.dat not found".into())
}

fn summary(p: &Progs) {
    let d = Disasm::new(p);
    println!("version {}  crc {}  entityfields {}", p.version, p.crc, p.entityfields);
    println!(
        "{} statements, {} functions, {} globals ({} defs), {} fields, {} bytes of strings",
        p.statements.len(), p.functions.len(), p.globals.len(), p.globaldefs.len(), p.fielddefs.len(), p.strings.len()
    );
    let mut files: Vec<String> = p
        .functions
        .iter()
        .filter(|f| f.first_statement > 0)
        .map(|f| String::from_utf8_lossy(p.string(f.file)).into_owned())
        .collect();
    files.dedup();
    files.sort();
    files.dedup();
    println!("files: {}", files.join(" "));
    println!("builtins:");
    let mut b: Vec<usize> = (0..p.functions.len()).filter(|&i| p.functions[i].first_statement < 0).collect();
    b.sort_by_key(|&i| -p.functions[i].first_statement);
    for i in b {
        println!("  {}", d.header(i));
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    if it.next().map(String::as_str) != Some("progs") {
        return Err(USAGE.into());
    }
    let (mut file, mut games, mut funcs, mut all, mut grep) = (None, None, Vec::new(), false, None);
    while let Some(a) = it.next() {
        let mut val = || it.next().map(String::as_str).ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "-g" => games = Some(val()?),
            "--func" => funcs.push(val()?),
            "--grep" => grep = Some(val()?),
            "--all" => all = true,
            "-h" | "--help" => return Err(USAGE.into()),
            f if !f.starts_with('-') => file = Some(f),
            o => return Err(format!("unknown option {o}\n{USAGE}")),
        }
    }
    let data = load(file, games)?;
    let p = Progs::parse(&data).map_err(|e| e.to_string())?;
    let d = Disasm::new(&p);
    if all || grep.is_some() {
        for i in 1..p.functions.len() {
            let text = d.function(i);
            match grep {
                Some(g) if text.contains(g) => println!("{}", d.header(i)),
                Some(_) => {}
                None => println!("{text}"),
            }
        }
    } else if !funcs.is_empty() {
        for n in funcs {
            let i = p.find_function(n).ok_or_else(|| format!("no function {n}"))?;
            println!("{}", d.function(i));
        }
    } else {
        summary(&p);
    }
    Ok(())
}

fn main() -> ExitCode {
    // Exit quietly when piped into `head` instead of panicking on EPIPE.
    std::panic::set_hook(Box::new(|i| {
        if !i.to_string().contains("Broken pipe") {
            eprintln!("{i}");
        }
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
