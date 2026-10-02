use std::process::ExitCode;

use pp_lab::counter::{self, MemOrder};
use pp_lab::litmus;
use pp_lab::padded::CACHE_LINE;
use pp_lab::spsc;

fn usage() {
    eprintln!(
        r#"Использование: harness <команда> [аргументы]

Команды:
  info
  counter <T> <N> <relaxed|seqcst> [--cas]
  litmus <N> <relaxed|seqcst>
  spsc <capacity> <items>
  bench-counter <T> <N> <ordering>
  bench-spsc <capacity> <items>"#
    );
}

fn parse<T: std::str::FromStr>(text: &str, what: &str) -> Result<T, String> {
    text.parse::<T>()
        .map_err(|_| format!("аргумент {what}: ожидалось целое число, получено {text:?}"))
}

fn arg<'a>(rest: &'a [String], index: usize, what: &str) -> Result<&'a str, String> {
    rest.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("не указан аргумент {what}"))
}

fn per_second(count: u64, seconds: f64) -> f64 {
    count as f64 / seconds.max(1e-9)
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        usage();
        return Err("не указана команда".into());
    };
    let rest = &args[1..];

    match command {
        "info" => println!(r#"{{"lab":1,"language":"Rust","cache_line":{CACHE_LINE}}}"#),

        "counter" => {
            let threads: usize = parse(arg(rest, 0, "T")?, "T")?;
            let n: u64 = parse(arg(rest, 1, "N")?, "N")?;
            let order: MemOrder = arg(rest, 2, "ordering")?.parse()?;
            let cas = rest.iter().skip(3).any(|flag| flag == "--cas");
            let total = counter::run(threads, n, order, cas);
            let expected = threads as u64 * n;
            println!(
                r#"{{"total":{total},"expected":{expected},"ordering":"{}","cas":{cas}}}"#,
                order.name()
            );
        }

        "litmus" => {
            let n: u64 = parse(arg(rest, 0, "N")?, "N")?;
            let order: MemOrder = arg(rest, 1, "ordering")?.parse()?;
            let o = litmus::run(n, order);
            println!(
                r#"{{"iterations":{},"both_zero":{},"r1_only":{},"r2_only":{},"both_one":{}}}"#,
                o.iterations, o.both_zero, o.r1_only, o.r2_only, o.both_one
            );
        }

        "spsc" => {
            let capacity: usize = parse(arg(rest, 0, "capacity")?, "capacity")?;
            let items: u64 = parse(arg(rest, 1, "items")?, "items")?;
            let r = spsc::run(capacity, items);
            println!(
                r#"{{"popped":{},"sum":{},"ordered":{},"capacity":{},"seconds":{:.6}}}"#,
                r.popped,
                r.sum,
                r.ordered,
                r.capacity,
                r.elapsed.as_secs_f64()
            );
        }

        "bench-counter" => {
            let threads: usize = parse(arg(rest, 0, "T")?, "T")?;
            let n: u64 = parse(arg(rest, 1, "N")?, "N")?;
            let order: MemOrder = arg(rest, 2, "ordering")?.parse()?;
            let cas = rest.iter().skip(3).any(|flag| flag == "--cas");
            let (_, elapsed) = counter::run_timed(threads, n, order, cas);
            let ops = threads as u64 * n;
            let seconds = elapsed.as_secs_f64();
            println!(
                r#"{{"ops":{ops},"seconds":{seconds:.6},"ops_per_sec":{:.1}}}"#,
                per_second(ops, seconds)
            );
        }

        "bench-spsc" => {
            let capacity: usize = parse(arg(rest, 0, "capacity")?, "capacity")?;
            let items: u64 = parse(arg(rest, 1, "items")?, "items")?;
            let r = spsc::run(capacity, items);
            let seconds = r.elapsed.as_secs_f64();
            println!(
                r#"{{"items":{items},"seconds":{seconds:.6},"items_per_sec":{:.1}}}"#,
                per_second(items, seconds)
            );
        }

        other => {
            usage();
            return Err(format!("неизвестная команда: {other}"));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ошибка: {error}");
            ExitCode::from(2)
        }
    }
}
