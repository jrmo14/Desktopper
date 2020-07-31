use crate::backend::tasks::Task;
use crate::backend::{CompletionStatus, EstTime};
use chrono::Local;
use serde::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde::export::Formatter;
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::{HashMap, HashSet};
use std::fmt;
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct ToDo {
    tasks: HashMap<Uuid, Task>,
    // #[serde(skip)]
    categories: HashMap<Option<String>, Vec<Uuid>>,
    // #[serde(skip)]
    overdue: HashSet<Uuid>,
}

impl EstTime for ToDo {
    fn est_time(&self) -> u32 {
        self.tasks.values().map(|task| task.est_time()).sum()
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

impl Eq for ToDo {}

impl PartialEq for ToDo {
    fn eq(&self, other: &Self) -> bool {
        self.tasks.len() == other.tasks.len()
            && self.tasks.keys().all(|key| other.tasks.contains_key(key))
    }
}

impl ToDo {
    pub fn new() -> Self {
        ToDo {
            tasks: HashMap::new(),
            categories: HashMap::new(),
            overdue: HashSet::new(),
        }
    }

    pub fn from_map(task_map: HashMap<Uuid, Task>) -> Result<Self, &'static str> {
        // Make sure that all associations are legit
        for (key, value) in task_map.iter() {
            if *key != value.get_id() {
                return Err("Key and value don't match");
            }
        }
        let categories_list: Vec<Option<String>> = task_map
            .iter()
            .map(|(_key, value)| value.get_category())
            .collect();
        let mut categories: HashMap<Option<String>, Vec<Uuid>> = categories_list
            .into_iter()
            .map(|cat| (cat, vec![]))
            .collect();
        for (id, task) in task_map.iter() {
            categories.get_mut(&task.get_category()).unwrap().push(*id);
        }

        let overdue = task_map
            .iter()
            .filter(|pair| pair.1.overdue())
            .map(|pair| pair.1.get_id())
            .collect();
        Ok(ToDo {
            tasks: task_map,
            categories,
            overdue,
        })
    }

    // Can return self, because we know that the id -> task relations will be valid
    pub fn from_vec(tasks: Vec<Task>) -> Self {
        ToDo::from_map(
            tasks
                .into_iter()
                .map(|task| (task.get_id(), task))
                .collect(),
        )
        .unwrap()
    }

    pub fn add_task(&mut self, task: Task) {
        self.categories
            .entry(task.get_category())
            .or_insert_with(Vec::new);
        self.categories
            .get_mut(&task.get_category())
            .unwrap()
            .push(task.get_id());
        if task.get_due_date().is_some() && Local::now() >= task.get_due_date().unwrap() {
            self.overdue.insert(task.get_id());
        }
        self.tasks.insert(task.get_id(), task);
    }

    pub fn remove_task(&mut self, id: Uuid) -> Result<(), ()> {
        if !self.tasks.contains_key(&id) {
            Err(())
        } else {
            let category = self.tasks.get(&id).as_ref().unwrap().get_category();
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
        self.tasks.keys().copied().collect()
    }

    /// Returns the number of tasks currently in the todo list
    pub fn num_tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn mark_finished(&mut self, id: Uuid, finished: Option<bool>) -> Result<(), ()> {
        match self.tasks.get_mut(&id) {
            Some(task) => {
                task.set_done(match finished {
                    Some(b) => b,
                    None => false,
                });
                Ok(())
            }
            None => Err(()),
        }
    }

    pub fn set_overdue(&mut self, id: Uuid) -> Result<(), &'static str> {
        match self.tasks.get(&id) {
            Some(_) => {
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

impl<'de> Deserialize<'de> for ToDo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Tasks,
        };
        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;
                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                        formatter.write_str("`tasks`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: Error,
                    {
                        match value {
                            "tasks" => Ok(Field::Tasks),
                            _ => Err(E::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }
        struct TodoVisitor;

        impl<'de> Visitor<'de> for TodoVisitor {
            type Value = ToDo;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("struct ToDo")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let tasks = seq
                    .next_element()?
                    .ok_or_else(|| Error::invalid_length(0, &self))?;
                Ok(ToDo::from_vec(tasks))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut tasks = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Tasks => {
                            if tasks.is_some() {
                                return Err(Error::duplicate_field("tasks"));
                            }
                            tasks = Some(map.next_value()?);
                        }
                    }
                }
                let tasks = tasks.ok_or_else(|| Error::missing_field("tasks"))?;
                Ok(ToDo::from_vec(tasks))
            }
        }

        const FIELDS: &[&str] = &["tasks"];
        deserializer.deserialize_struct("ToDo", FIELDS, TodoVisitor)
    }
}

impl Serialize for ToDo {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ToDo", 1)?;
        state.serialize_field(
            "tasks",
            &self
                .tasks
                .iter()
                .map(|pair| pair.1.clone())
                .collect::<Vec<Task>>(),
        )?;
        state.end()
    }
}

#[cfg(test)]
mod test {
    use crate::backend::{Task, ToDo};

    #[test]
    fn test_from_vec() {
        let task_a = Task::new(
            "Test1",
            "Test1",
            None,
            0,
            None,
            None,
            Some("testing".to_string()),
        );
        let task_b = Task::new(
            "Test2",
            "Test2",
            None,
            0,
            None,
            None,
            Some("testing".to_string()),
        );
        let task_c = Task::new(
            "Test3",
            "Test3",
            None,
            0,
            None,
            None,
            Some("testing_cat_2".to_string()),
        );
        let task_d = Task::new(
            "Test4",
            "Test4",
            None,
            0,
            None,
            None,
            Some("testing_cat_2".to_string()),
        );
        let test_vec = vec![
            task_a.clone(),
            task_b.clone(),
            task_c.clone(),
            task_d.clone(),
        ];
        let test_todo = ToDo::from_vec(test_vec);
        let mut ref_todo = ToDo::new();
        ref_todo.add_task(task_a);
        ref_todo.add_task(task_b);
        ref_todo.add_task(task_c);
        ref_todo.add_task(task_d);
        assert_eq!(test_todo, ref_todo);
    }

    #[test]
    fn serialize_deserialize() {
        let task_a = Task::new(
            "Test1",
            "Test1",
            None,
            0,
            None,
            None,
            Some("testing".to_string()),
        );
        let task_b = Task::new(
            "Test2",
            "Test2",
            None,
            0,
            None,
            None,
            Some("testing".to_string()),
        );
        let task_c = Task::new(
            "Test3",
            "Test3",
            None,
            0,
            None,
            None,
            Some("testing_cat_2".to_string()),
        );
        let task_d = Task::new(
            "Test4",
            "Test4",
            None,
            0,
            None,
            None,
            Some("testing_cat_2".to_string()),
        );
        let test_todo = ToDo::from_vec(vec![
            task_a.clone(),
            task_b.clone(),
            task_c.clone(),
            task_d.clone(),
        ]);
        let ser_str = serde_json::to_string(&test_todo).unwrap();
        let deserialized: ToDo = serde_json::from_str(ser_str.as_str()).unwrap();
        assert_eq!(deserialized, test_todo)
    }
}
