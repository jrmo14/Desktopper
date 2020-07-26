// TODO refactor function layout to make more sense

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use chrono::format::Numeric::WeekdayFromMon;
use chrono::prelude::{DateTime, Local, Weekday};
use chrono::{Datelike, Duration, FixedOffset};
use openssl_sys::DSA_up_ref;
use serde::{Deserialize, Serialize};
use std::ops::Add;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Default, Deserialize, Serialize, Debug)]
pub struct ToDo {
    tasks: HashMap<Uuid, Task>,
    categories: HashMap<Option<String>, Vec<Uuid>>,
    overdue: HashSet<Uuid>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Task {
    name: String,
    desc: String,
    finished: bool,
    due_date: Option<DateTime<Local>>,
    initial_date: DateTime<Local>,
    est_time: u32,
    priority: Option<Priority>,
    repeat: Option<Vec<Weekday>>,
    category: Option<String>,
    id: Uuid,
}

pub trait EstTime {
    fn est_time(&self) -> u32;
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug)]
pub enum Priority {
    Low,
    Medium,
    High,
    Extreme,
}

pub trait CompletionStatus {
    fn complete(&self) -> bool;
    fn completion_status(&self) -> (u32, u32);
}

impl ToDo {
    pub fn new() -> Self {
        ToDo {
            tasks: HashMap::new(),
            categories: HashMap::new(),
            overdue: HashSet::new(),
        }
    }

    pub fn add_task(&mut self, task: Task) {
        if !self.categories.contains_key(&task.category) {
            self.categories.insert(task.category.clone(), vec![]);
        }
        self.categories
            .get_mut(&task.category)
            .unwrap()
            .push(task.id);
        if task.due_date.is_some() && Local::now() >= task.due_date.unwrap() {
            self.overdue.insert(task.id);
        }
        self.tasks.insert(task.id, task);
    }

    pub fn remove_task(&mut self, id: Uuid) -> Result<(), ()> {
        if !self.tasks.contains_key(&id) {
            Err(())
        } else {
            let category = self.tasks.get(&id).as_ref().unwrap().category.clone();
            let cat_ids = self.categories.get_mut(&category).unwrap();
            for i in (0..cat_ids.len()).rev() {
                if cat_ids[i] == id {
                    cat_ids.remove(i);
                }
            }
            self.overdue.remove(&id);
            Ok(())
        }
    }

    pub fn get_task(&self, id: Uuid) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn get_task_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    pub fn get_all_tasks(&self) -> Vec<&Task> {
        self.tasks.values().collect()
    }

    pub fn get_all_tasks_mut(&mut self) -> Vec<&mut Task> {
        self.tasks.values_mut().collect()
    }

    /// Get all the tasks in a category
    pub fn get_category(&self, category: Option<String>) -> Option<Vec<&Task>> {
        match self.categories.get(&category) {
            Some(categories) => Some(
                categories
                    .iter()
                    .map(|id| self.tasks.get(id).unwrap())
                    .collect(),
            ),
            None => None,
        }
    }

    pub fn get_category_completion(&self, category: Option<String>) -> (usize, usize) {
        match self.categories.get(&category) {
            Some(category) => (
                category
                    .iter()
                    .map(|id| self.tasks.get(id).unwrap())
                    .filter(|task| task.complete())
                    .count(),
                category.len(),
            ),
            None => (0, 0),
        }
    }

    pub fn get_categories(&self) -> Vec<String> {
        self.categories
            .keys()
            .filter(|key| key.is_some())
            .map(|key| key.as_ref().unwrap().clone())
            .collect()
    }

    pub fn get_ids(&self) -> Vec<Uuid> {
        self.tasks.keys().map(|key| *key).collect()
    }

    /// Returns the number of tasks currently in the todo list
    pub fn num_tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn mark_finished(&mut self, id: Uuid, finished: Option<bool>) -> Result<(), ()> {
        match self.tasks.get_mut(&id) {
            Some(task) => Ok(task.set_done(if finished.is_some() {
                finished.unwrap()
            } else {
                true
            })),
            None => Err(()),
        }
    }

    pub fn set_overdue(&mut self, id: Uuid) -> Result<(), &'static str> {
        match self.tasks.get(&id) {
            Some(task) => {
                self.overdue.insert(id);
                Ok(())
            }
            None => Err("No task with that id"),
        }
    }

    pub fn get_overdue(&self) -> Vec<&Task> {
        self.overdue
            .iter()
            .map(|id| self.tasks.get(id).unwrap())
            .collect()
    }
}

impl CompletionStatus for ToDo {
    fn complete(&self) -> bool {
        self.tasks.values().all(|task| task.complete())
    }

    fn completion_status(&self) -> (u32, u32) {
        (
            self.tasks
                .values()
                .filter(|&task_box| task_box.complete())
                .count() as u32,
            self.tasks.values().len() as u32,
        )
    }
}

impl Task {
    pub fn new(
        name: &str,
        desc: &str,
        due_date: Option<DateTime<Local>>,
        est_time: u32,
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
            est_time,
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
                    if today_num > target_num {
                        // Anything that requires passing Sunday
                        (7 - today_num + target_num) as i64
                    } else if target_num > today_num {
                        // Sometime this calendar week
                        (target_num - today_num) as i64
                    } else {
                        // Same day of the week
                        7 as i64
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

impl EstTime for ToDo {
    fn est_time(&self) -> u32 {
        self.tasks.values().map(|task| task.est_time()).sum()
    }
}

impl EstTime for Task {
    fn est_time(&self) -> u32 {
        self.est_time
    }
}

impl Eq for ToDo {}

impl PartialEq for ToDo {
    fn eq(&self, other: &Self) -> bool {
        self.tasks.len() == other.tasks.len()
            && self.tasks.keys().all(|key| other.tasks.contains_key(key))
    }
}

impl Priority {
    pub fn val(self) -> i32 {
        match self {
            Priority::Low => 1,
            Priority::Medium => 2,
            Priority::High => 3,
            Priority::Extreme => 4,
        }
    }
}

impl FromStr for Priority {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match &(s.to_ascii_lowercase())[..] {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "mid" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            "extreme" => Ok(Priority::Extreme),
            _ => Err("Invalid Priority"),
        }
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.val().cmp(&other.val())
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.val().cmp(&other.val()))
    }
}

impl Eq for Priority {}

impl PartialEq for Priority {
    fn eq(&self, other: &Self) -> bool {
        self.val() == other.val()
    }
}

#[cfg(test)]
mod test {
    use chrono::{Duration, Local, Weekday};
}
