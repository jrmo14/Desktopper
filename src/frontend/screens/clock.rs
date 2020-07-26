use crate::frontend::{Buttons, Screen};
use chrono::{Date, Local};
use gpio_lcd::scheduler::{Job, ThreadedLcd};
use std::time::Duration;

pub struct ClockScreen {
    date: Date<Local>,
}

impl ClockScreen {
    pub fn new() -> Self {
        ClockScreen {
            date: Local::today(),
        }
    }
}

impl Default for ClockScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ClockScreen {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        let time = Local::now().format("%I:%M %p").to_string();
        self.date = Local::today();
        let date = self.date.format("%a %v").to_string();
        lcd.clear_jobs();
        lcd.add_job(Job::new(date.as_str(), 0, None));
        lcd.add_job(Job::new(time.as_str(), 1, None));
    }

    fn update_screen(&mut self, lcd: &mut ThreadedLcd, _buttons: Buttons) {
        self.tick(lcd)
    }

    fn get_tick(&self) -> Option<Duration> {
        Some(Duration::from_secs(1))
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
