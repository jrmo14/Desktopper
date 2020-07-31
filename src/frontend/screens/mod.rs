use std::time::Duration;

use gpio_lcd::scheduler::{Job, ThreadedLcd};

use crate::frontend::buttons::Buttons;

pub mod clock;
pub mod music;
pub mod tasks;

/// Defines generic behavior that each screens, to be displayed on an LCD/Character Display
pub trait Screen {
    /// Runs when the screen is switched into
    fn first_load(&mut self, lcd: &mut ThreadedLcd);
    /// Process button input, and potentially update screen content
    fn update_screen(&mut self, lcd: &mut ThreadedLcd, buttons: Buttons);
    /// Use this to say that the screen has an update without input, set to None to indicate that there is no tick action, defaults to returning None
    fn get_tick(&self) -> Option<Duration> {
        None
    }
    /// The Screen's tick action, update contents without button input i.e. a clock or music status, doesn't do anything by default
    fn tick(&mut self, _lcd: &mut ThreadedLcd) {}

    fn get_name(&self) -> String;
}

/// Keeps track of the Screens that will be put on the Display
pub struct DisplayState {
    screens: Vec<Box<dyn Screen>>,
    idx: usize,
    lcd: ThreadedLcd,
}

impl DisplayState {
    /// Create a new DisplayState with an ThreadedLcd controller
    pub fn new(lcd: ThreadedLcd) -> Self {
        DisplayState {
            screens: Vec::new(),
            idx: 0,
            lcd,
        }
    }

    /// Add a new screen
    pub fn add(&mut self, disp: Box<dyn Screen>) {
        self.screens.push(disp)
    }

    /// Select the next Screen for display
    pub fn next(&mut self) {
        self.idx = (self.idx + 1) % self.screens.len();
        self.screens[self.idx].first_load(&mut self.lcd);
        info!("Now showing {}", self.screens[self.idx].get_name());
    }

    /// Returns the Screen currently on display
    pub fn cur(&self) -> &dyn Screen {
        self.screens[self.idx].as_ref()
    }

    /// Calls the current screen's tick
    pub fn tick(&mut self) {
        self.screens[self.idx].as_mut().tick(&mut self.lcd)
    }

    ///  Runs the current Screen's update function
    pub fn update(&mut self, buttons: Buttons) {
        self.screens[self.idx]
            .as_mut()
            .update_screen(&mut self.lcd, buttons)
    }
}

pub struct TestScreen {}

impl Screen for TestScreen {
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        lcd.clear_jobs();
        lcd.clear_row(0);
        lcd.clear_row(1);
        lcd.add_job(Job::new("HELLO", 0, None));
    }

    fn update_screen(&mut self, lcd: &mut ThreadedLcd, _buttons: Buttons) {
        self.first_load(lcd)
    }

    fn get_name(&self) -> String {
        "Test".to_string()
    }
}
