use std::time::Duration;

use rppal::gpio::{Gpio,OutputPin,InputPin};

const TRAVEL_DISTANCE_FACTOR:f64 = 0.95;
const POLL_DELAY:Duration = Duration::from_millis(10);
const TIMEOUT:u16 = 300;
const MOTOR_ENABLE_ADDR:u8 = 22;
const MOTOR_DIRECTION_ADDR:u8 = 27;
const PISTON_ADDR:u8 = 25;
const RUN_SWITCH_ADDR:u8 = 10;
const UPPER_LIMIT_ADDR:u8 = 23;
const LOWER_LIMIT_ADDR:u8 = 24;

pub struct Fixture{
    gpio_api:Gpio,
    travel_distance: u32,
    safe_to_run: bool,
    motor_direction:OutputPin,
    motor_enable: OutputPin,
    piston_enable: OutputPin,
    upper_limit: InputPin,
    lower_limit: InputPin,
    run_switch: InputPin
}

impl Fixture{
    pub fn new() -> Self{
        let mut gpio = Gpio::new().unwrap();
        let output = Self{
            gpio_api:gpio,
            safe_to_run:true,
            travel_distance: u32::MAX,
            motor_direction: gpio.get(MOTOR_DIRECTION_ADDR).unwrap().into_output_low(),
            motor_enable: gpio.get(MOTOR_ENABLE_ADDR).unwrap().into_output_low(),
            piston_enable: gpio.get(PISTON_ADDR).unwrap().into_output_low(),
            upper_limit: gpio.get(UPPER_LIMIT_ADDR).unwrap().into_input_pulldown(),
            lower_limit: gpio.get(LOWER_LIMIT_ADDR).unwrap().into_input_pulldown(),
            run_switch: gpio.get(RUN_SWITCH_ADDR).unwrap().into_input_pulldown(),
        };
        output.run_switch.set_async_interrupt(rppal::gpio::Trigger::RisingEdge, move||{
            output.safe_to_run = false;
            while output.run_switch.is_low() {}
            output.safe_to_run = true;
        });
    }

    fn reset_arm(&mut self) -> &mut Self{
        //if(self.upper_limit.
    }
}
