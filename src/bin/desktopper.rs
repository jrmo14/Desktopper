#[macro_use]
extern crate log;
extern crate pretty_env_logger;
#[macro_use]
extern crate anyhow;

use clap::{App, Arg};
use desktopper::frontend::buttons::{Buttons, InputHandler};
use desktopper::frontend::screens::*;
use gpio_cdev::EventType::FallingEdge;
use gpio_cdev::*;
use gpio_lcd::lcd::LcdDriver;
use gpio_lcd::scheduler::ThreadedLcd;
use std::str::FromStr;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    // Always enable some form of logging
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();
    let matches = App::new("Desktopper")
        .arg(
            Arg::with_name("api_host")
                .short("a")
                .long("api_host")
                .value_name("API_HOST")
                .default_value("localhost")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("api_port")
                .short("p")
                .long("api_port")
                .long("API_PORT")
                .default_value("3030")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("chip")
                .short("c")
                .long("chip")
                .value_name("CHIP")
                .help("Sets the chip to use for GPIO")
                .default_value("/dev/gpiochip0")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("four_bit_mode")
                .short("f")
                .long("mode")
                .help("Sets the bit mode for the LCD panel")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("rs")
                .long("rs")
                .value_name("RS_PIN")
                .help("The pin to use for rs")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("rw")
                .long("rw")
                .value_name("RW_PIN")
                .help("The pin to use for rw")
                .default_value("255")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("enable")
                .short("e")
                .long("enable")
                .value_name("ENABLE_PIN")
                .help("The pin to use for enable")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("data_pins")
                .short("d")
                .long("data_pins")
                .value_name("DATA_PINS")
                .help("The 4/8 data pins")
                .multiple(true)
                .required(true),
        )
        .arg(
            Arg::with_name("mode_button")
                .short("m")
                .long("mode_button")
                .value_name("MODE_PIN")
                .help("The pin to change/reset the mode of the system")
                .required(true),
        )
        .arg(
            Arg::with_name("cycle_button")
                .long("cycle_button")
                .value_name("CYCLE_PIN")
                .help("The pin to cycle values in the system")
                .required(true),
        )
        .arg(
            Arg::with_name("fn_button_0")
                .long("fn_button_0")
                .value_name("BUTTON_0")
                .help("THe pin to use a function button 0")
                .required(true),
        )
        .arg(
            Arg::with_name("fn_button_1")
                .long("fn_button_1")
                .value_name("BUTTON_1")
                .help("THe pin to use a function button 1")
                .required(true),
        )
        .arg(
            Arg::with_name("fn_button_2")
                .long("fn_button_2")
                .value_name("BUTTON_2")
                .help("THe pin to use a function button 2")
                .required(true),
        )
        .get_matches();

    let mut chip = Chip::new(matches.value_of("chip").unwrap())?;

    let data_pins_res: Vec<Result<u8, std::num::ParseIntError>> = matches
        .values_of("data_pins")
        .unwrap()
        .map(|p| u8::from_str(p))
        .collect();

    let mut data_pins = Vec::new();

    if data_pins_res.len() != 8 && data_pins_res.iter().any(|res| res.is_err()) {
        return Err(anyhow!("Invalid number of data_pins, must be 4 or 8"));
    }
    data_pins_res.iter().for_each(|pin_res| {
        data_pins.push(pin_res.as_ref().unwrap());
    });

    let lcd_driver = LcdDriver::new(
        16,
        2,
        matches.value_of("chip").unwrap(),
        true,
        u8::from_str(matches.value_of("rs").unwrap()).unwrap(),
        u8::from_str(matches.value_of("rw").unwrap()).unwrap(),
        u8::from_str(matches.value_of("enable").unwrap()).unwrap(),
        *data_pins[0],
        *data_pins[1],
        *data_pins[2],
        *data_pins[3],
        *data_pins[4],
        *data_pins[5],
        *data_pins[6],
        *data_pins[7],
    )?;

    let scheduled_lcd = ThreadedLcd::with_driver(lcd_driver);

    let (tx, rx) = mpsc::channel();

    let mut input_handler = InputHandler::new(
        &mut chip,
        tx,
        u32::from_str(matches.value_of("mode_button").unwrap()).unwrap(),
        u32::from_str(matches.value_of("cycle_button").unwrap()).unwrap(),
        u32::from_str(matches.value_of("fn_button_0").unwrap()).unwrap(),
        u32::from_str(matches.value_of("fn_button_1").unwrap()).unwrap(),
        u32::from_str(matches.value_of("fn_button_2").unwrap()).unwrap(),
    );

    input_handler.start();

    let mut display_state = ScreenState::new(scheduled_lcd);
    display_state.add(Box::new(TaskDisplay::new(
        matches.value_of("api_host").unwrap(),
        matches.value_of("api_port").unwrap(),
    )));

    display_state.add(Box::new(TestDisplay {}));
    display_state.add(Box::new(ClockDisplay::new()));

    let mut button_state: Option<Buttons>;

    loop {
        if let Some(tick_dur) = display_state.cur().get_tick() {
            let mut end = Instant::now().checked_add(tick_dur).unwrap();
            loop {
                button_state = match rx.try_recv() {
                    Ok(buttons) => Some(buttons),
                    Err(e) => {
                        if e == TryRecvError::Disconnected {
                            error!("Input handler receive failed with: {}", e);
                        }
                        None
                    }
                };
                if button_state.is_some() {
                    break;
                }
                if end <= Instant::now() {
                    display_state.tick();
                    end = Instant::now()
                        .checked_add(display_state.cur().get_tick().unwrap())
                        .unwrap();
                }
            }
        } else {
            button_state = match rx.recv() {
                Ok(buttons) => Some(buttons),
                Err(e) => {
                    error!("Input handler receive failed with: {}", e);
                    None
                }
            }
        }
        if let Some(buttons) = button_state {
            if buttons.mode.state == Some(FallingEdge) {
                display_state.next()
            } else {
                display_state.update(buttons)
            }
        }
    }
    Ok(())
}
