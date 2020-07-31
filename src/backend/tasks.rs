// TODO refactor function layout to make more sense

use crate::backend::{CompletionStatus, EstTime, Priority};
use chrono::prelude::{DateTime, Local, Weekday};
use chrono::{Datelike, Duration};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Task {
    name: String,
    desc: String,
    finished: bool,
    due_date: Option<DateTime<Local>>,
    initial_date: DateTime<Local>,
    est_minutes: u32,
    priority: Option<Priority>,
    repeat: Option<Vec<Weekday>>,
    category: Option<String>,
    id: Uuid,
}

impl Task {
    pub fn new(
        name: &str,
        desc: &str,
        due_date: Option<DateTime<Local>>,
        est_minutes: u32,
        priority: Option<Priority>,
        repeat: Option<Vec<Weekday>>,
        category: Option<String>,
    ) -> Self {
        Task {
            name: name.to_string(),
            desc: desc.to_string(),
            finished: false,
            due_date,
            initial_date: Local::now(),
            est_minutes,
            priority,
            repeat,
            category,
            id: Uuid::new_v4(),
        }
    }

    pub fn overdue(&self) -> bool {
        if self.due_date.is_some() {
            self.due_date.unwrap() < Local::now()
        } else {
            false
        }
    }

    // Returns false if there is  no repeat
    pub fn repeat(&mut self) -> bool {
        if self.repeat.is_some() && self.due_date.is_some() {
            let today = Local::today().weekday();
            let mut diffs: Vec<i64> = self
                .repeat
                .as_ref()
                .unwrap()
                .iter()
                .map(|weekday| {
                    let today_num = today.num_days_from_monday();
                    let target_num = weekday.num_days_from_monday();
                    match today_num.cmp(&target_num) {
                        // Anything that requires passing Sunday
                        Ordering::Greater => (7 - today_num + target_num) as i64,
                        // Sometime this calendar week
                        Ordering::Equal => (target_num - today_num) as i64,
                        // Same day of the week
                        Ordering::Less => 7 as i64,
                    }
                })
                .collect();
            diffs.sort();
            // Only care about the day difference, not the number of hours or whatever
            let new_due_date = self.due_date.unwrap() + Duration::days(diffs[0]);
            self.due_date = Some(new_due_date);
            self.set_done(false);
            true
        } else {
            false
        }
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_desc(&self) -> String {
        self.desc.clone()
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_due_date(&self) -> Option<DateTime<Local>> {
        self.due_date
    }

    pub fn get_priority(&self) -> Option<Priority> {
        self.priority
    }

    pub fn get_category(&self) -> Option<String> {
        self.category.clone()
    }

    pub fn get_repeats(&self) -> Option<Vec<Weekday>> {
        self.repeat.clone()
    }

    pub fn set_done(&mut self, finished: bool) {
        self.finished = finished
    }
}

impl CompletionStatus for Task {
    fn complete(&self) -> bool {
        self.finished
    }

    fn completion_status(&self) -> (u32, u32) {
        (if self.finished { 1 } else { 0 }, 1)
    }
}

impl EstTime for Task {
    fn est_time(&self) -> u32 {
        self.est_minutes
    }
}

#[cfg(test)]
mod test {
    use chrono::{Duration, Local, Weekday};
}
