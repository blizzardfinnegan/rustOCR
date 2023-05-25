mod gpio_facade;
mod image_facade;
mod output_facade;
use std::{fs,path::Path,io::{stdin,stdout}};
use chrono::{DateTime,Local};
use gpio_facade::{Fixture,Direction};

const VERSION:&str = "0.0.0-alpha.1";
const DEFAULT_ITERATIONS:u64 = 10;

fn main() {
    setup_logs();
    log::info!("Rust OCR version {}",VERSION);
    let mut serials_set = false;
    let mut cameras_configured = false;
    let mut iteration_count = DEFAULT_ITERATIONS;

    //Initialise fixture
    let mut fixture:Option<Fixture> = None;
    loop {
        let possible_fixture = Fixture::new();
        match possible_fixture {
            Ok(fixture_object) => { 
                fixture = Some(fixture_object);
                break; 
            },
            _ => {
                print!("Fixture initialisation failed! Press enter to try again.");
                let mut user_input = String::new();
                stdin().read_line(&mut user_input).expect("Failed user input");
                let clean_input = user_input.trim();
                if clean_input.contains("override"){
                    break;
                }
            }
        }
    }
    
}

fn setup_logs() {
    let chrono_now:DateTime<Local> = Local::now();
    if !Path::new("logs").is_dir(){ _ = fs::create_dir("logs"); }
    _ = fern::Dispatch::new()
        .format(|out,message,record|{
            out.finish(format_args!(
                "{}  [{}, {}] - {}",
                Local::now().to_rfc3339(),
                record.level(),
                record.target(),
                message
            ))
        })
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Trace)
                .chain(fern::log_file(format!("logs/{}.log",
                    chrono_now.format("%Y-%m-%d_%H.%M").to_string())).unwrap()
                )
        )
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Info)
                .chain(stdout())
        )
        .apply();
}
