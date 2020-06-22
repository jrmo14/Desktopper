// TODO refactor function layout to make more sense

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::prelude::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Default, Deserialize, Serialize, Debug)]
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

pub trait CompletionStatus {
    fn complete(&self) -> bool;
    fn completion_status(&self) -> (i32, i32);
}

pub trait FlattenTasks {
    fn flatten_tasks(&self) -> Vec<Task>;
}

impl ToDo {
    pub fn new() -> Self {
        ToDo {
            tasks: HashMap::new(),
            paths: HashMap::new(),
        }
    }

    pub fn add_task(&mut self, id_parent: Option<Uuid>, task: Task) -> Result<(), &'static str> {
        match id_parent {
            Some(parent_id) => {
                let path = match self.paths.get(&parent_id.clone()) {
                    Some(path) => path,
                    None => return Err("Unable to find parent"),
                };

                let mut path_iter = path.iter();
                let mut parent = self.tasks.get_mut(path_iter.next().unwrap()).unwrap();
                for next_uuid in path_iter {
                    parent = parent
                        .subtasks
                        .as_mut()
                        .unwrap()
                        .get_mut(next_uuid)
                        .unwrap();
                }
                // Grab the parent so it's ours and we can mutate it safely
                parent.add_task(task.clone());
                debug!("Parent: {:?}", parent);
                let mut new_path = path.clone();
                new_path.push(task.uuid.clone());
                self.paths.insert(task.uuid.clone(), new_path);
            }
            None => {
                self.tasks.insert(task.uuid.clone(), Box::new(task.clone()));
                self.paths.insert(task.uuid.clone(), vec![task.uuid]);
            }
        }
        debug!("{:?}", self);
        Ok(())
    }

    pub fn remove_task(&mut self, id: Uuid) -> Result<Option<Task>, &'static str> {
        // Make sure to grab one above the parent
        let mut path_iter = match self.paths.get(&id) {
            Some(path) => path.as_slice()[0..path.len() - 1].iter(),
            None => return Err("No path for id"),
        };
        match path_iter.next() {
            Some(root_id) => {
                let mut parent = self.tasks.get_mut(root_id).unwrap();
                for next_id in path_iter {
                    parent = parent.subtasks.as_mut().unwrap().get_mut(next_id).unwrap();
                }
                Ok(parent.remove_subtask_task(id))
            }
            None => Ok(self.tasks.remove(&id).map(|task_box| (&*task_box).clone())),
        }
    }

    pub fn get_task(&self, id: Uuid) -> Option<&Task> {
        let mut path_iter = match self.paths.get(&id) {
            Some(path) => path.iter(),
            None => return None,
        };
        let mut task = self.tasks.get(path_iter.next().unwrap());
        for next_id in path_iter {
            task = task.unwrap().subtasks.as_ref().unwrap().get(next_id);
        }
        task.map(|task_box| task_box.as_ref())
    }

    pub fn get_task_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        let mut path_iter = match self.paths.get_mut(&id) {
            Some(path) => path.iter(),
            None => return None,
        };
        let mut task = self.tasks.get_mut(path_iter.next().unwrap());
        for next_task in path_iter {
            task = task.unwrap().subtasks.as_mut().unwrap().get_mut(next_task);
        }
        task.map(|task_box| task_box.as_mut())
    }

    pub fn get_all_tasks(&self) -> Vec<&Task> {
        self.tasks
            .values()
            .map(|task_box| task_box.as_ref())
            .collect()
    }

    pub fn get_all_tasks_mut(&mut self) -> Vec<&mut Task> {
        self.tasks
            .values_mut()
            .map(|task_box| task_box.as_mut())
            .collect()
    }
}

impl CompletionStatus for ToDo {
    fn complete(&self) -> bool {
        self.tasks.values().all(|task| task.complete())
    }

    fn completion_status(&self) -> (i32, i32) {
        unimplemented!()
    }
}

impl CompletionStatus for Task {
    fn complete(&self) -> bool {
        self.finished
            && match &self.subtasks {
                Some(subtasks) => subtasks.values().all(|task| task.complete()),
                None => true,
            }
    }

    fn completion_status(&self) -> (i32, i32) {
        let subtask_status = match &self.subtasks {
            Some(subtasks) => {
                let mut x = (0, 0);
                subtasks
                    .values()
                    .map(|task| task.completion_status())
                    .for_each(|status| {
                        x.0 += status.0;
                        x.1 += status.1;
                    });
                x
            }
            None => (0, 0),
        };
        (
            subtask_status.0 + if self.finished { 1 } else { 0 },
            subtask_status.1 + 1,
        )
    }
}

impl FlattenTasks for ToDo {
    fn flatten_tasks(&self) -> Vec<Task> {
        self.tasks
            .values()
            .map(|task| task.flatten_tasks())
            .flatten()
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

    pub fn remove_subtask_task(&mut self, id: Uuid) -> Option<Task> {
        match self.subtasks.as_mut() {
            Some(subtasks) => subtasks.remove(&id).map(|task_box| (&*task_box).clone()),
            None => None,
        }
    }

    pub fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_desc(&self) -> String {
        self.desc.clone()
    }

    pub fn get_due_date(&self) -> Option<DateTime<Local>> {
        self.due_date
    }

    pub fn get_priority(&self) -> Option<Priority> {
        self.priority
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

impl FlattenTasks for Task {
    fn flatten_tasks(&self) -> Vec<Task> {
        let mut ret = Vec::new();
        let tmp = Task {
            name: self.name.clone(),
            desc: self.desc.clone(),
            finished: self.finished,
            subtasks: None,
            due_date: self.due_date,
            _est_time: self._est_time,
            priority: self.priority,
            uuid: self.uuid,
        };
        ret.push(tmp);
        let subtasks = match &self.subtasks {
            Some(st) => {
                let top_lvl = st.values().map(|task| Task {
                    name: task.name.clone(),
                    desc: task.desc.clone(),
                    finished: task.finished,
                    subtasks: None,
                    due_date: task.due_date,
                    _est_time: task._est_time,
                    priority: task.priority,
                    uuid: task.uuid,
                });
                let children = st
                    .values()
                    .map(|children_st_box| children_st_box.flatten_tasks())
                    .flatten();
                top_lvl.chain(children).collect()
            }
            None => Vec::new(),
        };
        for x in subtasks.into_iter() {
            ret.push(x);
        }
        ret
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
            None => other.subtasks.is_none(),
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
