use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

pub mod tasks;
pub mod todo;
pub mod trello_api;

pub use tasks::Task;
pub use todo::ToDo;

pub trait EstTime {
    fn est_time(&self) -> u32;
}

pub trait CompletionStatus {
    fn complete(&self) -> bool;
    fn completion_status(&self) -> (u32, u32);
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug)]
pub enum Priority {
    Low,
    Medium,
    High,
    Extreme,
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

impl PartialEq for Priority {
    fn eq(&self, other: &Self) -> bool {
        self.val() == other.val()
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

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.val().cmp(&other.val()))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.val().cmp(&other.val())
    }
}

impl Eq for Priority {}
