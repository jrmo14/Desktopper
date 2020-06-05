use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::prelude::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::borrow::{Borrow, BorrowMut};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
pub struct ToDo {
    tasks: HashMap<Uuid, Box<Task>>,
    paths: HashMap<Uuid, Vec<Uuid>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Task {
    name: String,
    desc: String,
    finished: bool,
    subtasks: Option<HashMap<Uuid, Box<Task>>>,
    due_date: Option<DateTime<Local>>,
    _est_time: i32,
    priority: Option<Priority>,
    uuid: Uuid,
}

pub trait EstTime {
    fn est_time(&self) -> i32;
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug)]
pub enum Priority {
    Low,
    Medium,
    High,
    Extreme,
}

impl ToDo {
    pub fn new() -> Self {
        ToDo {
            tasks: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, uuid_parent: Option<Uuid>, task: Task) -> Result<(), &'static str> {
        match uuid_parent {
            Some(parent_uuid) => {
                let path = match self.paths.get(&parent_uuid.clone()) {
                    Some(path) => path,
                    None => return Err("Unable to find parent"),
                };

                let mut path_iter = path.iter();
                let mut parent = self.tasks.get_mut(path_iter.next().unwrap()).unwrap();
                while let Some(next_uuid) = path_iter.next() {
                    parent = parent
                        .subtasks
                        .as_mut()
                        .unwrap()
                        .get_mut(next_uuid)
                        .unwrap();
                }
                // Grab the parent so it's ours and we can mutate it safely
                parent.add_task(task.clone());
                info!("Parent: {:?}", parent);
                let mut new_path = path.clone();
                new_path.push(task.uuid.clone());
                self.paths.insert(task.uuid.clone(), new_path);
            }
            None => {
                self.tasks.insert(task.uuid.clone(), Box::new(task.clone()));
                self.paths.insert(task.uuid.clone(), vec![task.uuid]);
            }
        }
        info!("tasks: {:?}", self.tasks);
        info!("paths: {:?}", self.paths.keys());
        debug!("{:?}", self);
        Ok(())
    }

    pub fn get_task(&self, uuid: Uuid) -> Option<Task> {
        let path = match self.paths.get(&uuid) {
            Some(p) => p,
            None => return None,
        };
        let mut t = self.tasks.get(&path[0]);
        for idx in 1..path.len() {
            t = t.unwrap().subtasks.as_ref().unwrap().get(&path[idx]);
        }
        Some(t.unwrap().as_ref().clone())
    }

    pub fn get_all_tasks(&self) -> Vec<Task> {
        self.tasks
            .values()
            .map(|task_box| (&**task_box).clone())
            .collect()
    }
}

impl EstTime for ToDo {
    fn est_time(&self) -> i32 {
        self.tasks.values().map(|task| task.est_time()).sum()
    }
}

impl Eq for ToDo {}

impl PartialEq for ToDo {
    fn eq(&self, other: &Self) -> bool {
        self.tasks.len() == other.tasks.len()
            && self.tasks.keys().all(|key| other.tasks.contains_key(key))
    }
}

impl Task {
    pub fn new(
        name: &str,
        desc: &str,
        subtasks: Option<Vec<Task>>,
        due_date: Option<DateTime<Local>>,
        _est_time: i32,
        priority: Option<Priority>,
    ) -> Self {
        Task {
            name: name.to_string(),
            desc: desc.to_string(),
            finished: false,
            subtasks: match subtasks {
                Some(tasks) => Some(
                    tasks
                        .iter()
                        .map(|task| (task.get_uuid(), Box::new(task.clone())))
                        .collect(),
                ),
                None => Some(HashMap::new()),
            },
            due_date,
            _est_time,
            priority,
            uuid: Uuid::new_v4(),
        }
    }

    pub fn add_task(&mut self, subtask: Task) {
        match self.subtasks {
            Some(_) => {}
            None => self.subtasks = Some(HashMap::new()),
        }
        // Just yolo unwrap b/c we know that its been made by now
        self.subtasks
            .as_mut()
            .unwrap()
            .insert(subtask.uuid, Box::new(subtask));
    }

    pub fn get_uuid(&self) -> Uuid {
        self.uuid
    }
}

impl EstTime for Task {
    fn est_time(&self) -> i32 {
        match &self.subtasks {
            Some(subtasks) => {
                subtasks.values().map(|task| task.est_time()).sum::<i32>() + self._est_time
            }
            None => self._est_time,
        }
    }
}

impl Priority {
    pub fn val(&self) -> i32 {
        match self {
            Priority::Low => 1,
            Priority::Medium => 2,
            Priority::High => 3,
            Priority::Extreme => 4,
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
        let subtasks_match = match &self.subtasks {
            Some(tasks) => match &other.subtasks {
                Some(other_tasks) => tasks
                    .keys()
                    .map(|key| other_tasks.keys().any(|other_key| other_key == key))
                    .all(|x| x),
                None => false,
            },
            None => match &other.subtasks {
                Some(_) => false,
                None => true,
            },
        };
        subtasks_match
            && self.name == other.name
            && self.desc == other.desc
            && self._est_time == other._est_time
            && self.finished == other.finished
            && self.priority == other.priority
            && self.due_date == other.due_date
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
    use chrono::{Duration, Local};

    use super::{EstTime, Priority, Task, ToDo};
    use std::boxed::Box;
    use std::rc::Box;

    #[test]
    fn task_sort_priority() {
        let ref_vec = vec![
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Low)),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Medium)),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::High)),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Extreme)),
        ];
        let mut test_vec = vec![
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Medium)),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::High)),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Extreme)),
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new("TEST!", "TEST!", None, None, 0, Some(Priority::Low)),
        ];
        test_vec.sort_by(|a, b| a.priority.cmp(&b.priority));
        assert_eq!(test_vec, ref_vec);
    }

    #[test]
    fn task_sort_due_date() {
        let now = Local::now();
        let ref_vec = vec![
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new("TEST!", "TEST!", None, Some(now), 0, None),
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::minutes(1)),
                0,
                None,
            ),
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::hours(1)),
                0,
                None,
            ),
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::hours(5)),
                0,
                None,
            ),
        ];

        let mut test_vec = vec![
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::hours(5)),
                0,
                None,
            ),
            Task::new("TEST!", "TEST!", None, Some(now), 0, None),
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::minutes(1)),
                0,
                None,
            ),
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new(
                "TEST!",
                "TEST!",
                None,
                Some(now + Duration::hours(1)),
                0,
                None,
            ),
        ];

        test_vec.sort();
        assert_eq!(ref_vec, test_vec);
    }

    #[test]
    fn task_sort_est_time() {
        let ref_vec = vec![
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new("TEST!", "TEST!", None, None, 1, Some(Priority::Low)),
            Task::new("TEST!", "TEST!", None, None, 2, Some(Priority::Medium)),
            Task::new("TEST!", "TEST!", None, None, 3, Some(Priority::High)),
            Task::new("TEST!", "TEST!", None, None, 4, Some(Priority::Extreme)),
        ];
        let mut test_vec = vec![
            Task::new("TEST!", "TEST!", None, None, 2, Some(Priority::Medium)),
            Task::new("TEST!", "TEST!", None, None, 4, Some(Priority::Extreme)),
            Task::new("TEST!", "TEST!", None, None, 1, Some(Priority::Low)),
            Task::new("TEST!", "TEST!", None, None, 0, None),
            Task::new("TEST!", "TEST!", None, None, 3, Some(Priority::High)),
        ];

        test_vec.sort_by(|a, b| a.est_time().cmp(&b.est_time()));
        assert_eq!(ref_vec, test_vec);
    }

    #[test]
    fn subtask_sort_est_time() {
        let subtask_a = vec![
            Task::new("TEST3", "TEST!", None, None, 3, Some(Priority::High)),
            Task::new("TEST4", "TEST!", None, None, 4, Some(Priority::Extreme)),
        ];

        let ref_vec = vec![
            Task::new("TEST1", "TEST!", None, None, 1, Some(Priority::Low)),
            Task::new("TEST2", "TEST!", None, None, 2, Some(Priority::Medium)),
            Task::new("TEST0", "TEST!", Some(subtask_a), None, 0, None), // Task 0 has the greatest estimated time when factoring sub tasks
        ];

        let mut test_vec = vec![ref_vec[0].clone(), ref_vec[2].clone(), ref_vec[1].clone()];

        test_vec.sort_by(|a, b| a.est_time().cmp(&b.est_time()));
        assert_eq!(ref_vec, test_vec);
    }

    #[test]
    fn task_subtask_insert() {
        let subtask = Task::new("SUB_TEST", "SUB_TEST", None, None, 0, None);
        let ref_task = Task::new("TEST", "TEST", Some(vec![subtask.clone()]), None, 0, None);
        let mut test_task = Task {
            name: "TEST".to_string(),
            desc: "TEST".to_string(),
            finished: false,
            subtasks: None,
            due_date: None,
            _est_time: 0,
            priority: None,
            uuid: ref_task.get_uuid(),
        };
        test_task.add_task(subtask.clone());
        assert_eq!(test_task, ref_task);
    }

    #[test]
    fn todo_insert_test() {
        let task0 = Task::new("TEST0", "TEST0", None, None, 0, None);
        let task1 = Task::new("TEST1", "TEST1", None, None, 1, None);
        let mut ref_todo = ToDo::new();
        ref_todo.tasks.insert(task0.uuid, Box::new(task0.clone()));
        ref_todo.tasks.insert(task1.uuid, Box::new(task1.clone()));
        let mut test_todo = ToDo::new();
        assert_eq!(test_todo.add_task(None, task0.clone()), Ok(()));
        assert_eq!(test_todo.add_task(None, task1.clone()), Ok(()));

        assert_eq!(ref_todo, test_todo);
    }

    #[test]
    fn todo_subtask_insert() {
        let subtask = Task::new("SUB_TEST", "SUB_TEST", None, None, 0, None);
        let mut ref_task = Task::new("TEST", "TEST", None, None, 0, None);
        ref_task.add_task(subtask.clone());

        let test_task = Task {
            name: "TEST".to_string(),
            desc: "TEST".to_string(),
            finished: false,
            subtasks: None,
            due_date: None,
            _est_time: 0,
            priority: None,
            uuid: ref_task.get_uuid(),
        };

        let mut ref_todo = ToDo::new();
        let mut test_todo = ToDo::new();
        assert_eq!(ref_todo.add_task(None, ref_task), Ok(()));

        let test_uuid = test_task.get_uuid();
        assert_eq!(test_todo.add_task(None, test_task), Ok(()));
        assert_eq!(test_todo.add_task(Some(test_uuid), subtask.clone()), Ok(()));

        assert_eq!(ref_todo, test_todo);
    }
}
