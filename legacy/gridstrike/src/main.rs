use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use gridstrike::{CellAddr, CellInput, Workbook};

const USAGE: &str = "GridStrike local worksheet\n\
  gridstrike new FILE.gstrike SHEET_NAME\n\
  gridstrike set FILE.gstrike A1 text|number|formula VALUE\n\
  gridstrike clear FILE.gstrike A1\n\
  gridstrike get FILE.gstrike A1\n\
  gridstrike undo FILE.gstrike\n\
  gridstrike redo FILE.gstrike\n\
  gridstrike csv FILE.gstrike\n\
  gridstrike csv-raw FILE.gstrike (exact text; may be interpreted as formulas)";

fn main() -> ExitCode {
    match run(&std::env::args().collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("GridStrike: {error}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let command = args.get(1).ok_or("missing command")?.as_str();
    if command == "help" || command == "--help" {
        println!("{USAGE}");
        return Ok(());
    }
    let path = Path::new(args.get(2).ok_or("missing workbook path")?);
    if command == "new" {
        exact(args, 4)?;
        if path.exists() {
            return Err("workbook already exists; choose a new path".into());
        }
        let name = &args[3];
        if name.trim().is_empty() || name.len() > 100 {
            return Err("sheet name must be 1 to 100 bytes".into());
        }
        Workbook::new(name).save_new(path)?;
        println!("Created {}", path.display());
        return Ok(());
    }
    let mut book = Workbook::open(path)?;
    let sheet = book.active_sheet();
    match command {
        "set" => {
            exact(args, 6)?;
            let address = CellAddr::parse(&args[3])?;
            let input = match args[4].as_str() {
                "text" => CellInput::Text(args[5].clone()),
                "number" => {
                    CellInput::Number(args[5].parse().map_err(|_| "value must be a number")?)
                }
                "formula" => CellInput::Formula(args[5].clone()),
                _ => return Err("cell type must be text, number, or formula".into()),
            };
            book.set(sheet, address, input)?;
            book.save(path)?;
            println!("{}", book.value(sheet, address));
        }
        "clear" => {
            exact(args, 4)?;
            book.set(sheet, CellAddr::parse(&args[3])?, CellInput::Empty)?;
            book.save(path)?;
            println!("Cleared.");
        }
        "get" => {
            exact(args, 4)?;
            let address = CellAddr::parse(&args[3])?;
            println!("{}\t{}", address.a1(), book.value(sheet, address));
        }
        "undo" => {
            exact(args, 3)?;
            book.undo()?;
            book.save(path)?;
            println!("Undone.");
        }
        "redo" => {
            exact(args, 3)?;
            book.redo()?;
            book.save(path)?;
            println!("Redone.");
        }
        "csv" => {
            exact(args, 3)?;
            std::io::stdout()
                .write_all(book.export_csv(sheet)?.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        "csv-raw" => {
            exact(args, 3)?;
            std::io::stdout()
                .write_all(book.export_csv_raw(sheet)?.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        _ => return Err(format!("unknown command: {command}")),
    }
    Ok(())
}

fn exact(args: &[String], count: usize) -> Result<(), String> {
    if args.len() == count {
        Ok(())
    } else {
        Err(format!("expected {} arguments", count - 1))
    }
}
