use chrono::prelude::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Deserialize, Serialize, Debug)]
struct Project {
    name: String,
    desc: String,
    tasks: Vec<Task>,
    due_date: Option<DateTime<Local>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct Task {
    name: String,
    desc: String,
    finished: bool,
    subtasks: Option<Vec<Task>>,
    due_date: Option<DateTime<Local>>,
    est_time: i32,
}

impl Project {
    pub fn new(
        name: String,
        desc: String,
        tasks: Vec<Task>,
        due_date: Option<DateTime<Local>>,
    ) -> Self {
        Project {
            name,
            desc,
            tasks,
            due_date,
        }
    }
}

impl Task {
    pub fn new(
        name: String,
        desc: String,
        subtasks: Option<Vec<Task>>,
        due_date: Option<DateTime<Local>>,
        est_time: i32,
    ) -> Self {
        Task {
            name,
            desc,
            subtasks,
            due_date,
            est_time,
            finished: false,
        }
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.due_date.cmp(&other.due_date)
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Task {}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.due_date == other.due_date
    }
}

#[cfg(test)]
mod test {
    use super::{Project, Task};
}
