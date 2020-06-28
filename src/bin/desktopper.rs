#[macro_use]
extern crate log;
extern crate pretty_env_logger;
#[macro_use]
extern crate anyhow;

use clap::{App, Arg};
use desktopper::tasks::*;
use gpio_cdev::*;
use gpio_lcd::lcd::LcdDriver;
use gpio_lcd::scheduler::{Job, ThreadedLcd};
use nix::poll::{poll, PollFd, PollFlags};
use reqwest::blocking::Client;
use std::os::unix::io::AsRawFd;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

struct State {
    displays: Vec<Box<dyn Display>>,
    idx: usize,
}

impl State {
    pub fn new() -> Self {
        State {
            displays: Vec::new(),
            idx: 0,
        }
    }
}

trait Display {
    fn first_load(&mut self, lcd: &mut ThreadedLcd);
    fn update_display(
        &mut self,
        lcd: &mut ThreadedLcd,
        cycle_btn: bool,
        btn_0: bool,
        btn_1: bool,
        btn_2: bool,
    );
}

struct TaskDisplay {
    client: Client,
    cur_id: Option<Uuid>,
    idx: usize,
    tasks: Vec<Task>,
    api_root: String,
}

impl TaskDisplay {
    pub fn new(api_host: &str, api_port: &str) -> Self {
        let client = Client::new();
        let api_root = format!("http://{}:{}", api_host, api_port);
        let url = format!("{}/todo/get", &api_root);
        let resp_str = client.get(&url).send().unwrap().text().unwrap();
        let tasks = serde_json::from_str(&resp_str).unwrap();
        TaskDisplay {
            client,
            cur_id: None,
            idx: 0,
            tasks,
            api_root,
        }
    }
    pub fn update_tasks(&mut self) {
        let url = format!(
            "{}/todo/get/{}",
            &self.api_root,
            match self.cur_id {
                Some(id) => format!("?id={}", id),
                None => String::new(),
            }
        );
        let resp = self.client.get(&url).send().unwrap().text().unwrap();
        self.tasks = serde_json::from_str(&resp).unwrap();
    }
}

impl Display for TaskDisplay {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        lcd.clear_jobs();
        self.update_tasks();
        if !self.tasks.is_empty() {
            let top = self.tasks[self.idx].get_name();
            let bottom = self.tasks[self.idx].get_desc();
            lcd.clear_jobs();
            lcd.add_job(Job::new(top.as_str(), 0, Some(Duration::from_millis(250))));
            lcd.add_job(Job::new(
                bottom.as_str(),
                1,
                Some(Duration::from_millis(250)),
            ));
        }
    }

    fn update_display(
        &mut self,
        lcd: &mut ThreadedLcd,
        cycle_btn: bool,
        _btn_0: bool,
        _btn_1: bool,
        _btn_2: bool,
    ) {
        if cycle_btn {
            self.idx = (self.idx + 1) % self.tasks.len();
            let top = self.tasks[self.idx].get_name();
            let bottom = self.tasks[self.idx].get_desc();
            lcd.clear_jobs();
            lcd.add_job(Job::new(top.as_str(), 0, Some(Duration::from_millis(250))));
            lcd.add_job(Job::new(
                bottom.as_str(),
                1,
                Some(Duration::from_millis(250)),
            ));
        }
    }
}

struct TestDisplay {}

impl Display for TestDisplay {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        lcd.clear_jobs();
        lcd.clear_row(0);
        lcd.clear_row(1);
        lcd.add_job(Job::new("HELLO", 0, None));
    }

    fn update_display(
        &mut self,
        lcd: &mut ThreadedLcd,
        _cycle_btn: bool,
        _btn_0: bool,
        _btn_1: bool,
        _btn_2: bool,
    ) {
        self.first_load(lcd)
    }
}

fn main() -> anyhow::Result<()> {
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

    let mut scheduled_lcd = ThreadedLcd::with_driver(lcd_driver);

    let mut button_fds = Vec::new();
    let mut event_handles = Vec::new();

    create_poll_fd(
        &mut chip,
        u32::from_str(matches.value_of("mode_button").unwrap()).unwrap(),
        &mut button_fds,
        &mut event_handles,
    );

    create_poll_fd(
        &mut chip,
        u32::from_str(matches.value_of("cycle_button").unwrap()).unwrap(),
        &mut button_fds,
        &mut event_handles,
    );

    create_poll_fd(
        &mut chip,
        u32::from_str(matches.value_of("fn_button_0").unwrap()).unwrap(),
        &mut button_fds,
        &mut event_handles,
    );

    create_poll_fd(
        &mut chip,
        u32::from_str(matches.value_of("fn_button_1").unwrap()).unwrap(),
        &mut button_fds,
        &mut event_handles,
    );

    create_poll_fd(
        &mut chip,
        u32::from_str(matches.value_of("fn_button_2").unwrap()).unwrap(),
        &mut button_fds,
        &mut event_handles,
    );

    let mut state = State::new();
    state.displays.push(Box::new(TaskDisplay::new(
        matches.value_of("api_host").unwrap(),
        matches.value_of("api_port").unwrap(),
    )));
    state.displays.push(Box::new(TestDisplay {}));
    // TODO Add debouncing somehow
    loop {
        if poll(&mut button_fds, -1)? == 0 {
            println!("Timeout?!?!?");
        } else {
            for i in 0..button_fds.len() {
                if let Some(revts) = button_fds[i].revents() {
                    let h = &mut event_handles[i];
                    if revts.contains(PollFlags::POLLIN) {
                        let event = h.get_event()?;
                        let offset = h.line().offset();
                        if offset == 27 {
                            info!("Changing display");
                            state.idx = (state.idx + 1) % state.displays.len();
                            state.displays[state.idx].first_load(&mut scheduled_lcd)
                        } else {
                            info!("Updating display state");
                            let cycle_state =
                                event.event_type() == EventType::FallingEdge && offset == 27;
                            state.displays[state.idx].update_display(
                                &mut scheduled_lcd,
                                cycle_state,
                                false,
                                false,
                                false,
                            );
                        }
                    } else if revts.contains(PollFlags::POLLPRI) {
                        println!("[{}] Got a POLLPRI", h.line().offset());
                    }
                }
            }
        }
    }
}

fn create_poll_fd(
    chip: &mut Chip,
    line_offset: u32,
    button_fds: &mut Vec<PollFd>,
    event_handles: &mut Vec<LineEventHandle>,
) {
    let line_handle = chip
        .get_line(line_offset)
        .unwrap()
        .events(
            LineRequestFlags::INPUT,
            EventRequestFlags::BOTH_EDGES,
            "desktopper",
        )
        .unwrap();
    button_fds.push(PollFd::new(
        (&line_handle).as_raw_fd(),
        PollFlags::POLLIN | PollFlags::POLLPRI,
    ));
    event_handles.push(line_handle);
}
