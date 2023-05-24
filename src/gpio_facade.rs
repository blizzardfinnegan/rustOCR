use std::time::Duration;
use async_std::sync::*;
use rppal::gpio::{Gpio,OutputPin,InputPin,Trigger,Level};
use std::thread;
use std::result::Result;
use futures::executor;
use std::fmt;

const TRAVEL_DISTANCE_FACTOR:f64 = 0.95;
const POLL_DELAY:Duration = Duration::from_millis(10);
const TIMEOUT:u16 = 300;
const MOTOR_ENABLE_ADDR:u8 = 22;
const MOTOR_DIRECTION_ADDR:u8 = 27;
const PISTON_ADDR:u8 = 25;
const RUN_SWITCH_ADDR:u8 = 10;
const UPPER_LIMIT_ADDR:u8 = 23;
const LOWER_LIMIT_ADDR:u8 = 24;

static MOVE_LOCK:RwLock<bool> = RwLock::new(true);

pub struct Fixture{
    gpio_api:Gpio,
    travel_distance: u32,
    motor_direction:Option<OutputPin>,
    motor_enable: Option<OutputPin>,
    piston_enable: Option<OutputPin>,
    upper_limit: Option<InputPin>,
    lower_limit: Option<InputPin>,
}

pub enum Direction{Up,Down}

impl Drop for Fixture{
    fn drop(&mut self) {
        self.reset_arm();
    }
}

#[derive(Debug,Clone)]
pub struct GpioError;
pub struct FixtureInitError;

impl fmt::Display for GpioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bad fixture GPIO settings")
    }
}
impl fmt::Display for FixtureInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid fixture PWM value")
    }
}

impl Fixture{
    //modify implementation to allow for multiple fixtures simultaneously
    pub fn new() -> Result<Self,GpioError>{
        let gpio = Gpio::new().unwrap();
        let mut output = Self{
            gpio_api:gpio,
            travel_distance: u32::MAX,
            motor_direction: None,
            motor_enable: None,
            piston_enable: None,
            upper_limit: None,
            lower_limit: None,
        };

        output.motor_enable = Some(output.gpio_api.get(MOTOR_ENABLE_ADDR).unwrap().into_output_low());
        output.motor_direction = Some(output.gpio_api.get(MOTOR_DIRECTION_ADDR).unwrap().into_output_low());
        output.piston_enable = Some(output.gpio_api.get(PISTON_ADDR).unwrap().into_output_low());
        output.upper_limit = Some(output.gpio_api.get(UPPER_LIMIT_ADDR).unwrap().into_input_pulldown());
        output.lower_limit = Some(output.gpio_api.get(LOWER_LIMIT_ADDR).unwrap().into_input_pulldown());

        let possible_run_pin = output.gpio_api.get(RUN_SWITCH_ADDR);
        match possible_run_pin{
            Ok(run_pin) =>{
                _ = run_pin.into_input_pulldown().set_async_interrupt(Trigger::Both, |switch_state|{
                    let mut move_allowed = executor::block_on(MOVE_LOCK.write());
                    match switch_state {
                        Level::Low=> {
                            *move_allowed = false;
                        }
                        Level::High => {
                            *move_allowed = true;
                        }
                        
                    };
                });
            },
            Err(_) => {}
        }

        while let Err(_) = output.find_distance(){};

        return Ok(output);
    }

    fn reset_arm(&mut self) -> u16{
        if let (Some(upper_limit),          Some(motor_direction_pin),      Some(motor_enable_pin)) = 
               (self.upper_limit.as_mut(), self.motor_direction.as_mut(), self.motor_enable.as_mut()){
            if upper_limit.is_high(){
                {
                    while !*executor::block_on(MOVE_LOCK.read()) {}
                    motor_direction_pin.set_low();
                    motor_enable_pin.set_high()
                }
                thread::sleep(Duration::from_millis(500));
                motor_enable_pin.set_low()
            }
            while !*executor::block_on(MOVE_LOCK.read()){}
            motor_direction_pin.set_high();
            motor_enable_pin.set_high();
            let mut counter = 0;
            for _ in 0..TIMEOUT {
                while !*executor::block_on(MOVE_LOCK.read()){
                    motor_enable_pin.set_low();
                }
                if *executor::block_on(MOVE_LOCK.read()) && motor_enable_pin.is_set_low(){
                    motor_enable_pin.set_high();
                }
                if upper_limit.is_high() { break; }
                counter += 1;
                thread::sleep(POLL_DELAY);
            }
            motor_enable_pin.set_low();
            if counter < TIMEOUT { return counter; }
        };
        return 0;
    }

    fn find_distance(&mut self) -> Result<(),FixtureInitError>{
        if self.reset_arm() == 0 { return Err(FixtureInitError); }
        let mut down_counter:u32 = 0;
        let mut up_counter:u32 = 0;

        //Time travel time to lower limit switch 
        if let (Some(lower_limit),            Some(motor_direction_pin),      Some(motor_enable_pin)) = 
               (self.lower_limit.as_mut(), self.motor_direction.as_mut(), self.motor_enable.as_mut()){
            while !*executor::block_on(MOVE_LOCK.read()){}
            motor_direction_pin.set_low();
            motor_enable_pin.set_high();
            for _ in 0..TIMEOUT{
                while !*executor::block_on(MOVE_LOCK.read()){
                    motor_enable_pin.set_low();
                }
                if *executor::block_on(MOVE_LOCK.read()) && motor_enable_pin.is_set_low(){
                    motor_enable_pin.set_high();
                }
                if lower_limit.is_high() { break; }
                down_counter += 1;
                thread::sleep(POLL_DELAY);
            }
        }

        //Time travel time to upper limit switch 
        if let (Some(upper_limit),            Some(motor_direction_pin),      Some(motor_enable_pin)) = 
               (self.upper_limit.as_mut(), self.motor_direction.as_mut(), self.motor_enable.as_mut()){
            while !*executor::block_on(MOVE_LOCK.read()){}
            motor_direction_pin.set_low();
            motor_enable_pin.set_high();
            for _ in 0..TIMEOUT{
                while !*executor::block_on(MOVE_LOCK.read()){
                    motor_enable_pin.set_low();
                }
                if *executor::block_on(MOVE_LOCK.read()) && motor_enable_pin.is_set_low(){
                    motor_enable_pin.set_high();
                }
                if upper_limit.is_high() { break; }
                up_counter += 1;
                thread::sleep(POLL_DELAY);
            }
        }

        self.travel_distance = std::cmp::min(up_counter,down_counter);

        return Ok(())
    }

    pub fn goto_limit(&mut self, direction:Direction) -> bool{
        let ref mut limit_sense:InputPin;
        match direction{
            Direction::Down => {
                if let Some(obj) = self.lower_limit.as_mut() {
                    limit_sense = obj;
                }
                else { return false; }
            },
            Direction::Up => {
                if let Some(obj) = self.upper_limit.as_mut() {
                    limit_sense = obj;
                }
                else { return false; }
            }
        }

        if limit_sense.is_high() { return true; }

        let move_polls = (self.travel_distance as f64 * TRAVEL_DISTANCE_FACTOR) as u64;

        if let (Some(motor_direction_pin),      Some(motor_enable_pin)) = 
               (self.motor_direction.as_mut(), self.motor_enable.as_mut()){
            while !*executor::block_on(MOVE_LOCK.read()){}
            motor_direction_pin.set_low();
            motor_enable_pin.set_high();
            for _ in 0..move_polls{
                while !*executor::block_on(MOVE_LOCK.read()){
                    motor_enable_pin.set_low();
                }
                if *executor::block_on(MOVE_LOCK.read()) && motor_enable_pin.is_set_low(){
                    motor_enable_pin.set_high();
                }
                if limit_sense.is_high() { break; }
                thread::sleep(POLL_DELAY);
            }
        }

        return limit_sense.is_high();
    }

    pub fn push_button(&mut self){
        if let Some(piston_enable) = self.piston_enable.as_mut(){
            while !*executor::block_on(MOVE_LOCK.read()){}
            piston_enable.set_high();
            thread::sleep(Duration::from_secs(1));
            piston_enable.set_low();
        }
    }
}
