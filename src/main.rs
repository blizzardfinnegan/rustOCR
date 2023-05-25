mod gpio_facade;
mod image_facade;
mod output_facade;
use std::{fs,path::Path,io::{stdin,stdout}, thread,time::Duration,sync::Arc};
use chrono::{DateTime,Local};
use gpio_facade::{Fixture,Direction};

use crate::{image_facade::{Camera, OCR}, output_facade::{TestState, OutputFile}};

const VERSION:&str = "0.0.0-alpha.1";
const DEFAULT_ITERATIONS:u64 = 10;
const CAMERA_FILE_PREFIX:&str = "video-cam-";

fn main() {
    setup_logs();
    log::info!("Rust OCR version {}",VERSION);
    let mut serials_set = false;
    let mut cameras_configured = false;
    let mut iteration_count = DEFAULT_ITERATIONS;

    //Initialise fixture
    let mut fixture:Option<Fixture> = None;
    loop {
        match Fixture::new() {
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

    //Initialise cameras
    let mut camera_init_threads = Vec::new();
    let mut available_cameras = Vec::new();
    match fs::read_dir("/dev"){
        Ok(dev_directory) =>{
            //There's a lot of files in the /dev directory, check each file in its own thread
            for device in dev_directory.into_iter(){
                match device{
                    Ok(safe_device)=>{
                        camera_init_threads.push(thread::spawn(move||{
                            if safe_device.file_name().to_string_lossy().contains(CAMERA_FILE_PREFIX){
                                Camera::new(safe_device.file_name().to_string_lossy().to_string())
                            }
                            else { None }
                        }));
                    },
                    Err(_)=>{}
                }
            }
            for thread in camera_init_threads{
                match thread.join(){
                    Ok(Some(camera)) => available_cameras.push(camera),
                    Ok(None) => {},
                    Err(error) =>{
                        log::warn!("Camera unavailable! Unplug all cameras, plug them back in, and restart this program.");
                        log::debug!("{:?}",error);
                    }
                }
            }
        },
        Err(error) =>{
            log::warn!("Could not open dev directory! Did you run with `sudo`?");
            log::debug!("{}",error);
        }
    }

    loop{
        print_main_menu(iteration_count,cameras_configured,serials_set);

        match get_user_number_input(){
            1 => {
                configure_cameras();
                cameras_configured = true;
            },
            2 => {
                configure_serials();
                serials_set = true;
            },
            3 => iteration_count = set_iteration_count(),
            4 => set_active_cameras(),
            5 => run_tests(&mut fixture,&mut available_cameras,cameras_configured,iteration_count),
            6 => print_main_menu_help(),
            7 => break,
            _ => log::warn!("Invalid user input! Please input a valid number.")
        }
    }
}

fn configure_cameras(){}
fn configure_serials(){}
fn set_iteration_count() -> u64{ return 0; }
fn set_active_cameras(){}
fn run_tests(fixture:&mut Option<Fixture>,available_cameras:&mut Vec<Camera>,cameras_configured:bool,iteration_count:u64) {
    log::info!("Initialising tests...");
    let mut buf = String::new();
    if !cameras_configured{
        log::warn!("You have not configured the cameras yet! Are you sure you would like to continue? (y/N): ");
        _ = stdin().read_line(&mut buf);
        if buf.is_empty() { return }
        else if ! buf.to_lowercase().contains('y'){ return }
        else { log::debug!("Using default camera behaviour. This will most likely cause bad data."); }
    }
    let mut serials_set:bool = true;
    let mut active_cameras = Vec::new();
    let mut camera_serials = Vec::new();
    for camera in available_cameras.iter(){
        if camera.is_active() && camera.get_serial().clone().trim().is_empty() {
            serials_set = false;
            break;
        }
        else if camera.is_active(){
            camera_serials.push(camera.get_serial().clone());
            active_cameras.push(camera);
        }
    }
    if !serials_set{
        log::warn!("You have not set serials for any devices yet! Are you sure you would like to continue? (y/N): ");
        _ = stdin().read_line(&mut buf);
        if buf.is_empty() { return }
        else if ! buf.to_lowercase().contains('y'){ return }
        else { log::debug!("Using empty serials. This behaviour is currently undefined."); }
    }

    if active_cameras.len() == 0{ 
        log::error!("No active cameras! Please make sure at least one camera is active before testing.");
        return
    }

    if let Some(safe_fixture) = fixture{
        safe_fixture.push_button();
    }

    let current_state = TestState::new(camera_serials.clone());

    let mut output_file = OutputFile::new(camera_serials);

    for i in 0..iteration_count{
        log::info!("Starting iteration {} of {}...",i+1,iteration_count);
        loop{
            if let Some(safe_fixture) = fixture{
                safe_fixture.goto_limit(Direction::Up);
                safe_fixture.goto_limit(Direction::Down);
                safe_fixture.push_button();
                thread::sleep(Duration::from_secs(2));
            }
            let mut threads = Vec::new();
            for camera in active_cameras.clone().into_iter(){
                let local_camera = camera.clone();
                threads.push(thread::spawn(move||{
                    local_camera.complete_process()
                }));
            }
            let mut ocr = OCR::new();
            let mut retry = false;
            for thread in threads.into_iter(){
                match thread.join(){
                    Ok(image_location) => {
                        let result = ocr.parse_image(image_location.clone());
                        if result < 10.0 || result > 100.0 {
                            retry = true;
                        }
                        for camera in active_cameras.clone().into_iter(){
                            if image_location.contains(&camera.get_serial().clone()){
                                current_state.add_iteration(camera.get_serial().clone(), result);
                                log::info!("Parsed image from camera {}: {}",
                                    camera.get_serial().clone(),result);
                            }
                        }
                    },
                    Err(_) =>{}
                }
            }
            if retry { 
                log::warn!("Bad OCR reading! Ignoring current values, and resetting DUTs...");
                if let Some(safe_fixture) = fixture{
                    safe_fixture.goto_limit(Direction::Up);
                    log::info!("Waiting for 20 seconds to allow devices to fall asleep.");
                    thread::sleep(Duration::from_secs(20));
                    safe_fixture.push_button();
                }
                continue; 
            } 
            else {
                output_file.write_values(&current_state, None, None);
                break;
            }
        }
    }
}

fn print_main_menu(iteration_count:u64,cameras_configured:bool,serials_set:bool){
    println!("\n\n");
    println!("===========================================");
    println!("Main menu:");
    println!("-------------------------------------------");
    println!("Current iteration count: {}",iteration_count);
    println!("-------------------------------------------");
    print!("1. Configure camera ");
    if cameras_configured {
        println!("[complete]");
    }
    else{
        println!("");
    };
    print!("2. Set serial numbers ");
    if serials_set {
        println!("[complete]");
    }
    else{
        println!("");
    };
    println!("3. Change iteration count");
    println!("4. Toggle active cameras");
    println!("5. Run tests");
    println!("6. Help");
    println!("7. Exit");
    println!("===========================================");
}
fn print_main_menu_help(){
    println!("\n\n");
    println!("===========================================");
    println!("Explanations:");
    println!("1. Configure camera ");
    println!("{}{}{}{}{}{}","\tChange values for the cameras to",
             "\n\tadjust image for use in OCR.",
             "\n\tAvaliable variables to change:",
             "\n\t\tcrop dimensions",
             "\n\t\tcomposite frame count",
             "\n\t\tthreshold value");
    println!("2. Set serial numbers ");
    println!("{}{}","\tSet the serial for each device",
             "\n\tbeing tested. This is used for data saving");
    println!("3. Change iteration count");
    println!("\tChange the number of times to test the devices.");
    println!("4. Toggle active cameras");
    println!("\tChange which cameras are active during testing.");
    println!("5. Run tests");
    println!("\tRun tests, with configured settings");
    println!("6. Help");
    println!("\tShow this help menu.");
    println!("7. Exit");
    println!("\tClose the program.");
    println!("===========================================");
    println!("Press enter to continue...");
    let mut buf = String::new();
    _ = stdin().read_line(&mut buf);
}

fn print_camera_menu(){
}

fn get_user_number_input() -> u64{
    let mut user_input:String = String::default();
    match stdin().read_line(&mut user_input){
        Ok(_) => {
            match user_input.trim().parse(){
                Ok(value)    => return value,
                Err(error)=>{
                    log::warn!("User input cannot be parsed!");
                    log::debug!("{}",error);
                }
            }
        },
        Err(error) => {
            log::warn!("Unable to read user input!");
            log::debug!("{}",error);
        }
    }
    return 0;
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
