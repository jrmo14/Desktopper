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

mod config {
    use serde::Deserialize;
    use std::io::BufReader;

    #[derive(Deserialize)]
    pub struct Config {
        pub gpio: GPIO,
        pub tasks: Tasks,
    }

    #[derive(Deserialize)]
    pub struct GPIO {
        pub chip_name: String,
        pub display: DisplayConfig,
        pub buttons: ButtonConfig,
    }
    #[derive(Deserialize)]
    pub struct DisplayConfig {
        pub rs: u8,
        pub enable: u8,
        pub data: [u8; 8],
        pub rw: u8,
        pub four_bit: bool,
    }

    #[derive(Deserialize)]
    pub struct ButtonConfig {
        pub mode: u32,
        pub cycle: u32,
        pub fn0: u32,
        pub fn1: u32,
        pub fn2: u32,
    }

    #[derive(Deserialize)]
    pub struct Tasks {
        pub host: String,
        pub port: String,
    }

    pub fn parse_file(file_location: &str) -> Config {
        toml::from_str(std::fs::read_to_string(file_location).unwrap().as_str()).unwrap()
    }
}

fn main() -> anyhow::Result<()> {
    // Always enable some form of logging
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();
    let matches = App::new("Desktopper")
        .arg(
            Arg::with_name("config_file")
                .short("c")
                .long("config")
                .default_value("/etc/desktopper/config.toml"),
        )
        .get_matches();

    let cfg = config::parse_file(matches.value_of("config_file").unwrap());

    let mut chip = Chip::new(cfg.gpio.chip_name.clone())?;

    let lcd_driver = LcdDriver::new(
        16,
        2,
        cfg.gpio.chip_name.as_str(),
        cfg.gpio.display.four_bit,
        cfg.gpio.display.rs,
        cfg.gpio.display.rw,
        cfg.gpio.display.enable,
        cfg.gpio.display.data[0],
        cfg.gpio.display.data[1],
        cfg.gpio.display.data[2],
        cfg.gpio.display.data[3],
        cfg.gpio.display.data[4],
        cfg.gpio.display.data[5],
        cfg.gpio.display.data[6],
        cfg.gpio.display.data[7],
    )?;

    let scheduled_lcd = ThreadedLcd::with_driver(lcd_driver);

    let (tx, rx) = mpsc::channel();

    let mut input_handler = InputHandler::new(
        &mut chip,
        tx,
        cfg.gpio.buttons.mode,
        cfg.gpio.buttons.cycle,
        cfg.gpio.buttons.fn0,
        cfg.gpio.buttons.fn1,
        cfg.gpio.buttons.fn2,
    );

    input_handler.start();

    let mut display_state = ScreenState::new(scheduled_lcd);
    display_state.add(Box::new(ClockDisplay::new()));
    display_state.add(Box::new(TaskDisplay::new(
        cfg.tasks.host.as_str(),
        cfg.tasks.port.as_str(),
    )));

    display_state.add(Box::new(TestDisplay {}));
    display_state.next();
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
