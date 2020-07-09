use crate::backend::tasks::Task;
use crate::frontend::buttons::Buttons;
use chrono::prelude::Local;
use chrono::Date;
use gpio_cdev::EventType::FallingEdge;
use gpio_lcd::scheduler::{Job, ThreadedLcd};
use reqwest::blocking::Client;
use std::time::Duration;
use uuid::Uuid;

pub trait Screen {
    fn first_load(&mut self, lcd: &mut ThreadedLcd);
    // Process button input
    fn update_display(&mut self, lcd: &mut ThreadedLcd, buttons: Buttons);
    // Use this to say that the display has an update without input, set to None to indicate that there is no tick action
    fn get_tick(&self) -> Option<Duration>;
    // Update without input i.e. a clock or music status
    fn tick(&mut self, lcd: &mut ThreadedLcd);
}

pub struct ScreenState {
    screens: Vec<Box<dyn Screen>>,
    idx: usize,
    lcd: ThreadedLcd,
}

impl ScreenState {
    pub fn new(lcd: ThreadedLcd) -> Self {
        ScreenState {
            screens: Vec::new(),
            idx: 0,
            lcd,
        }
    }

    pub fn add(&mut self, disp: Box<dyn Screen>) {
        self.screens.push(disp)
    }

    pub fn next(&mut self) {
        self.idx = (self.idx + 1) % self.screens.len();
        self.screens[self.idx].first_load(&mut self.lcd)
    }

    pub fn cur(&self) -> &dyn Screen {
        self.screens[self.idx].as_ref()
    }

    pub fn tick(&mut self) {
        self.screens[self.idx].as_mut().tick(&mut self.lcd)
    }

    pub fn update(&mut self, buttons: Buttons) {
        self.screens[self.idx]
            .as_mut()
            .update_display(&mut self.lcd, buttons)
    }
}

pub struct TaskDisplay {
    client: Client,
    cur_id: Option<Uuid>,
    parent_id: Option<Uuid>,
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
            parent_id: None,
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

impl Screen for TaskDisplay {
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
    // Need to document if falling edge is release or not
    fn update_display(&mut self, lcd: &mut ThreadedLcd, buttons: Buttons) {
        if buttons.cycle.state == Some(FallingEdge) {
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
        } else if buttons.f0.state == Some(FallingEdge) {
        }
    }

    fn get_tick(&self) -> Option<Duration> {
        None
    }

    // This does nothing
    fn tick(&mut self, _lcd: &mut ThreadedLcd) {}
}

pub struct TestDisplay {}

impl Screen for TestDisplay {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        lcd.clear_jobs();
        lcd.clear_row(0);
        lcd.clear_row(1);
        lcd.add_job(Job::new("HELLO", 0, None));
    }

    fn update_display(&mut self, lcd: &mut ThreadedLcd, _buttons: Buttons) {
        self.first_load(lcd)
    }

    fn get_tick(&self) -> Option<Duration> {
        None
    }

    fn tick(&mut self, _lcd: &mut ThreadedLcd) {}
}

pub struct ClockDisplay {
    date: Date<Local>,
}

impl ClockDisplay {
    pub fn new() -> Self {
        ClockDisplay {
            date: Local::today(),
        }
    }
}

impl Default for ClockDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ClockDisplay {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        let time = Local::now().format("%I:%M %p").to_string();
        self.date = Local::today();
        let date = self.date.format("%a %v").to_string();
        lcd.clear_jobs();
        lcd.add_job(Job::new(date.as_str(), 0, None));
        lcd.add_job(Job::new(time.as_str(), 1, None));
    }

    fn update_display(&mut self, lcd: &mut ThreadedLcd, _buttons: Buttons) {
        self.tick(lcd)
    }

    fn get_tick(&self) -> Option<Duration> {
        Some(Duration::from_millis(250))
    }

    fn tick(&mut self, lcd: &mut ThreadedLcd) {
        let time = Local::now().format("%I:%M %p").to_string();
        if self.date != Local::today() {
            self.date = Local::today();
            let date = self.date.format("%a %v").to_string();
            lcd.clear_jobs();
            lcd.add_job(Job::new(date.as_str(), 0, None));
        } else {
            lcd.clear_row(1);
        }
        lcd.add_job(Job::new(time.as_str(), 1, None));
    }
}
