#[macro_use]
extern crate log;
extern crate pretty_env_logger;

use std::fs::File;
use std::io::BufReader;
use std::ops::DerefMut;
use std::process;

use warp::Filter;

use crate::data_model::DataStore;

// TODO make into a user input
const SAVE_FILE_PATH: &str = "/etc/desktopper/todo.json";

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "INFO");
    }
    pretty_env_logger::init();

    info!("My pid is {}", process::id());

    let data_store = DataStore::new();
    match File::open(SAVE_FILE_PATH) {
        Ok(file) => {
            info!("Reading from {} file", SAVE_FILE_PATH);
            // TODO: Fix deserialization --> might want to load just the hashmap, and rebuild the overdue and category segments
            match serde_json::from_reader(BufReader::new(file)) {
                Ok(todo) => {
                    info!("Loaded todo from {}", SAVE_FILE_PATH);
                    *data_store.todo_list.write().deref_mut() = todo;
                }
                Err(_) => {
                    warn!("Unable to load from storage file, invalid data, will make a new one")
                }
            }
        }
        Err(_) => warn!("Unable to open save file, will create new one."),
    }
    let task_routes = filters::task_master(data_store);
    let todo_routes = task_routes.with(warp::log("todo"));
    warp::serve(todo_routes).run(([0, 0, 0, 0], 3030)).await;
}

mod data_model {
    use std::sync::Arc;

    use parking_lot::RwLock;

    use chrono::{DateTime, Local};
    use desktopper::backend::{CompletionStatus, ToDo};
    use tokio::task::JoinHandle;
    use tokio::{task, time};
    use uuid::Uuid;

    #[derive(Clone)]
    pub struct DataStore {
        pub todo_list: Arc<RwLock<ToDo>>,
    }

    impl DataStore {
        pub fn new() -> Self {
            DataStore {
                todo_list: Arc::new(RwLock::new(ToDo::new())),
            }
        }

        pub fn schedule_overdue_check(
            &self,
            id: Uuid,
            due_date: DateTime<Local>,
        ) -> JoinHandle<()> {
            let task_todo_list = self.todo_list.clone();
            task::spawn(async move {
                let dur = due_date.signed_duration_since(Local::now());
                time::delay_for(dur.to_std().unwrap()).await;
                let mut lock = task_todo_list.write();
                if let Some(task) = lock.get_task(id) {
                    if !task.complete() {
                        lock.set_overdue(id).unwrap();
                    }
                }
            })
        }

        pub fn schedule_repeats(&self, id: Uuid, due_date: DateTime<Local>) {
            let task_todo_list = self.todo_list.clone();
            task::spawn(async move {
                let mut keep_rep = true;
                while keep_rep {
                    let dur = due_date.signed_duration_since(Local::now());
                    time::delay_for(dur.to_std().unwrap()).await;
                    let mut lock = task_todo_list.write();
                    match lock.get_task_mut(id) {
                        // Task may have been removed
                        Some(task) => keep_rep = task.repeat(),
                        None => keep_rep = false,
                    }
                }
            });
        }
    }
}

mod filters {
    use std::collections::HashMap;
    use std::str::FromStr;

    use chrono::{DateTime, Local, TimeZone, Weekday};
    use serde::Deserialize;
    use uuid::Uuid;
    use warp::Filter;

    use crate::{handlers, DataStore};
    use desktopper::backend::{Priority, Task};

    pub fn task_master(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("todo").and(
            get_task(storage.clone())
                .or(add_task(storage.clone()))
                .or(remove_task(storage.clone()))
                .or(estimate_time(storage.clone()))
                .or(complete(storage.clone()))
                .or(completion_status(storage.clone()))
                .or(mark_finished(storage.clone()))
                .or(search(storage)),
        )
    }

    pub fn get_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("get"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::get_task)
    }

    pub fn add_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path("add"))
            .and(warp::path::end())
            .and(json_body())
            .and(with_store(storage))
            .and_then(handlers::add_task)
    }

    pub fn remove_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("delete"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::remove_task)
    }

    pub fn estimate_time(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("time"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::estimate_time)
    }

    pub fn complete(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("complete"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::complete)
    }

    pub fn completion_status(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("status"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(option_extractor::<String>("category"))
            .and(with_store(storage))
            .and_then(handlers::completion_status)
    }

    pub fn search(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("search"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(option_extractor::<String>("name"))
            .and(option_extractor::<String>("desc"))
            .and(option_extractor::<DateTime<Local>>("due_date_start"))
            .and(option_extractor::<DateTime<Local>>("due_date_end"))
            .and(option_extractor::<u32>("est_time_low"))
            .and(option_extractor::<u32>("est_time_high"))
            .and(option_extractor::<bool>("complete"))
            .and(option_extractor::<Priority>("priority_low"))
            .and(option_extractor::<Priority>("priority_high"))
            .and(option_extractor::<String>("category"))
            .and(with_store(storage))
            .and_then(handlers::search)
    }

    pub fn mark_finished(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("mark_finished"))
            .and(warp::path::end())
            .and(warp::query::<Uuid>())
            .and(option_extractor::<bool>("finished"))
            .and(with_store(storage))
            .and_then(handlers::mark_finished)
    }

    /// Extracts types that implement FromStr and wraps them in an Option
    /// If the key doesn't exist, then it's a none
    fn option_extractor<T: FromStr>(
        key: &str,
    ) -> impl Filter<Extract = (Option<T>,), Error = warp::Rejection> + Clone + '_ {
        warp::query::<HashMap<String, String>>().map(
            move |input: HashMap<String, String>| -> Option<T> {
                match input.get(key) {
                    Some(x) => match T::from_str(&*x) {
                        Ok(s) => Some(s),
                        _ => None,
                    },
                    None => None,
                }
            },
        )
    }

    fn with_store(
        storage: DataStore,
    ) -> impl Filter<Extract = (DataStore,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || storage.clone())
    }

    fn json_body() -> impl Filter<Extract = (Task,), Error = warp::Rejection> + Clone {
        // Make sure that we don't try to deserialize the uuid or sub-tasks
        // Want those to deserialize during loading from start, so can't mark to not afaik
        #[derive(Deserialize)]
        struct ApiTask {
            name: String,
            desc: String,
            due_date: Option<String>,
            est_time: u32,
            priority: Option<Priority>,
            repeat: Option<Vec<Weekday>>,
            category: Option<String>,
        }
        warp::body::content_length_limit(1024 * 16).and(warp::body::json::<ApiTask>().map(
            |x: ApiTask| -> Task {
                let new_due_date = match x.due_date {
                    Some(d) => {
                        match chrono::NaiveDateTime::parse_from_str(d.as_str(), "%Y-%m-%d %H:%M:%S")
                        {
                            Ok(time) => Some(Local.from_local_datetime(&time).unwrap()),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                Task::new(
                    x.name.as_str(),
                    x.desc.as_str(),
                    new_due_date,
                    x.est_time,
                    x.priority,
                    x.repeat,
                    x.category,
                )
            },
        ))
    }
}

mod handlers {
    use std::fs::OpenOptions;

    use uuid::Uuid;
    use warp::{http, Rejection};

    use crate::data_model::DataStore;
    use crate::SAVE_FILE_PATH;
    use chrono::{DateTime, Local};
    use desktopper::backend::{CompletionStatus, EstTime, Priority, Task};
    use std::ops::Deref;

    pub async fn add_task(
        task: Task,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        store.todo_list.write().add_task(task.clone());
        if task.get_due_date().is_some() {
            store.schedule_overdue_check(task.get_id(), task.get_due_date().unwrap());
        }
        if task.get_repeats().is_some() {
            store.schedule_repeats(
                task.get_id(),
                task.get_due_date().unwrap_or_else(Local::now),
            );
        }
        update_file(store);

        Ok(warp::reply::with_status(
            "Added task to todo list",
            http::StatusCode::CREATED,
        ))
    }

    pub async fn get_task(
        uuid: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        match uuid {
            Some(uuid) => match store.todo_list.read().get_task(uuid) {
                Some(task) => Ok(warp::reply::json(&task)),
                None => Err(warp::reject::not_found()),
            },
            None => {
                let read_store = store.todo_list.read();
                Ok(warp::reply::json(&*read_store))
            }
        }
    }

    pub async fn remove_task(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        let ret_val = match id {
            Some(id) => match store.todo_list.write().remove_task(id) {
                Ok(task) => Ok(warp::reply::json(&task)),
                Err(_) => Err(warp::reject::not_found()),
            },
            None => Err(warp::reject::reject()),
        };
        update_file(store);
        ret_val
    }

    pub async fn estimate_time(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        match id {
            Some(id) => match store.todo_list.read().get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.est_time())),
                None => Err(warp::reject()),
            },
            None => Ok(warp::reply::json(&store.todo_list.read().est_time())),
        }
    }

    pub async fn complete(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        match id {
            Some(id) => match store.todo_list.read().get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.complete())),
                None => Err(warp::reject()),
            },
            None => Ok(warp::reply::json(&store.todo_list.read().complete())),
        }
    }
    /// Get the completion status of either, the entire todo list,
    /// one category, or for a specific id
    pub async fn completion_status(
        id: Option<Uuid>,
        category: Option<String>,
        store: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        let store_lock = store.todo_list.read();
        if let Some(id) = id {
            match store_lock.get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.completion_status())),
                None => Err(warp::reject()),
            }
        } else if let Some(category) = category {
            // If there is a blank category passed, set it to none
            let list = if category.is_empty() {
                match store_lock.get_category(None) {
                    Some(list) => Ok(list),
                    None => Err(()),
                }
            } else {
                match store_lock.get_category(Some(category)) {
                    Some(list) => Ok(list),
                    None => Err(()),
                }
            };
            // The category doesn't exist
            if list.is_err() {
                Err(warp::reject())
            } else {
                // Grab the list itself and create our reply
                let list = list.unwrap();
                let num_complete = list.iter().filter(|task| task.complete()).count();
                Ok(warp::reply::json(&(num_complete, list.len())))
            }
        } else {
            // Just get the whole todo list's completion status
            Ok(warp::reply::json(&store_lock.completion_status()))
        }
    }
    /// Searches the todo list for tasks matching the search patterns.
    /// If ID is set, then the function will return the task with the matching id only,
    /// otherwise it will return a list of tasks that match the query(s).
    pub async fn search(
        id: Option<Uuid>,
        name: Option<String>,
        desc: Option<String>,
        due_date_start: Option<DateTime<Local>>,
        due_date_end: Option<DateTime<Local>>,
        est_time_low: Option<u32>,
        est_time_high: Option<u32>,
        complete: Option<bool>,
        priority_low: Option<Priority>,
        priority_high: Option<Priority>,
        category: Option<String>,
        storage: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        let todo_list = storage.todo_list.read();
        let search_results; // Pre-declare
        if let Some(id) = id {
            // fast path
            search_results = match todo_list.get_task(id) {
                Some(task) => vec![task],
                None => vec![], // dummy fast
            }
        } else {
            // ALL THE FILTERS
            search_results = todo_list
                .get_all_tasks()
                .into_iter()
                .filter(|task| match &name {
                    Some(name) => task.get_name().contains(name),
                    _ => true,
                })
                .filter(|task| match &desc {
                    Some(desc) => task.get_desc().contains(desc),
                    _ => true,
                })
                .filter(|task| match due_date_start {
                    // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                    Some(due_start) => task.get_due_date() >= Some(due_start),
                    _ => true,
                })
                .filter(|task| match due_date_end {
                    // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                    Some(due_end) => task.get_due_date() <= Some(due_end),
                    _ => true,
                })
                .filter(|task| match est_time_low {
                    Some(est_low) => task.est_time() >= est_low,
                    _ => true,
                })
                .filter(|task| match est_time_high {
                    Some(est_high) => task.est_time() <= est_high,
                    _ => true,
                })
                .filter(|task| match complete {
                    Some(complete) => {
                        if complete {
                            task.complete()
                        } else {
                            true
                        }
                    }
                    _ => true,
                })
                .filter(|task| match priority_low {
                    // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                    Some(priority) => task.get_priority() >= Some(priority),
                    _ => true,
                })
                .filter(|task| match priority_high {
                    // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                    Some(priority) => task.get_priority() <= Some(priority),
                    _ => true,
                })
                .filter(|task| match &category {
                    Some(category) => {
                        let wrapped_category = if category.is_empty() {
                            None
                        } else {
                            Some(category.clone())
                        };
                        task.get_category() == wrapped_category
                    }
                    _ => true,
                })
                .collect();
        }
        // Say hi
        Ok(warp::reply::json(&search_results))
    }

    pub async fn mark_finished(
        id: Uuid,
        finished: Option<bool>,
        store: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        match store.todo_list.write().mark_finished(id, finished) {
            Ok(()) => Ok(http::Response::builder().body(format!(
                "Set task {} to {}",
                id,
                if finished.is_some() {
                    finished.unwrap()
                } else {
                    true
                }
            ))),
            Err(()) => Err(warp::reject()),
        }
    }

    // TODO fix update_file to only serialize the hashmap that holds the tasks, not the categories or overdue as those are only to make searches and other features easier
    pub fn update_file(store: DataStore) {
        match serde_json::to_writer(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(SAVE_FILE_PATH)
                .unwrap(),
            store.todo_list.read().deref(),
        ) {
            Ok(_) => {}
            Err(e) => error!("{}", e),
        }
    }

    #[derive(Debug)]
    struct InvalidQuery;
    impl warp::reject::Reject for InvalidQuery {}
}
