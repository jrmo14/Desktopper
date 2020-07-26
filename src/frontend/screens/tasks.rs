use std::time::Duration;

use chrono::prelude::Local;
use chrono::Date;
use gpio_cdev::EventType::{self, FallingEdge, RisingEdge};
use gpio_lcd::scheduler::{Job, ThreadedLcd};
use reqwest::blocking::Client;
use uuid::Uuid;

use crate::backend::tasks::{CompletionStatus, Task, ToDo};
use crate::frontend::buttons::{Buttons, HELD, OPEN, RELEASED};
use crate::frontend::screens::Screen;

pub struct TaskScreen {
    client: Client,
    cur_id: Option<Uuid>,
    parent_id: Option<Uuid>,
    idx: usize,
    view_flag: usize,
    todo: ToDo,
    cur_category: Option<String>,
    api_root: String,
}

// TODO GET RID OF THE VALUES AND ONLY USE THE ENUM
enum TaskScreenState {
    Root,
    Categories,
    CategoryTasks,
    AllTasks,
    Overdue,
    TaskInfo,
}

impl TaskScreenState {
    pub fn val(self) -> usize {
        match self {
            TaskScreenState::Root => 0,
            TaskScreenState::Categories => 1,
            TaskScreenState::CategoryTasks => 2,
            TaskScreenState::AllTasks => 3,
            TaskScreenState::Overdue => 4,
            TaskScreenState::TaskInfo => 5,
        }
    }

    pub fn get(val: usize) -> TaskScreenState {
        match val % 6 {
            5 => TaskScreenState::TaskInfo,
            4 => TaskScreenState::Overdue,
            3 => TaskScreenState::AllTasks,
            2 => TaskScreenState::CategoryTasks,
            1 => TaskScreenState::Categories,
            _ => TaskScreenState::Root,
        }
    }
}

// TODO implement state machine
// 1. Show categories
// 2. Show task in a category
// 3. Show overdue tasks

impl TaskScreen {
    pub fn new(api_host: &str, api_port: &str) -> Self {
        let client = Client::new();
        let api_root = format!("http://{}:{}", api_host, api_port);
        let url = format!("{}/todo/get", &api_root);
        let resp_str = client.get(&url).send().unwrap().text().unwrap();
        let todo: ToDo = serde_json::from_str(&resp_str).unwrap();
        TaskScreen {
            client,
            cur_id: None,
            parent_id: None,
            idx: 0,
            view_flag: 0,
            todo,
            cur_category: None,
            api_root,
        }
    }
    pub fn update_tasks(&mut self) {
        let url = format!("{}/todo/get", &self.api_root,);
        let resp = self.client.get(&url).send().unwrap().text().unwrap();
        self.todo = serde_json::from_str(&resp).unwrap();
    }

    fn root_view(&mut self, lcd: &mut ThreadedLcd) {
        lcd.clear_jobs();
        self.update_tasks();
        self.idx = 0;
        self.view_flag = 0;
        let cs = self.todo.completion_status();
        lcd.add_job(Job::new(
            format!("Status: {}/{}", cs.0, cs.1).as_str(),
            0,
            None,
        ));
        lcd.add_job(Job::new(
            format!(
                "Overdue: {}, Categories: {}",
                self.todo.get_overdue().len(),
                self.todo.get_categories().len()
            )
            .as_str(),
            1,
            Some(Duration::from_millis(250)),
        ));
    }
}

impl Screen for TaskScreen {
    // TODO rewrite with new state machine
    fn first_load(&mut self, lcd: &mut ThreadedLcd) {
        self.root_view(lcd);
    }

    // TODO separate screen selection and formatting in to their own match blocks -> Display should be separate
    fn update_screen(&mut self, lcd: &mut ThreadedLcd, buttons: Buttons) {
        if buttons.cycle.state == RELEASED {
            // GO back to root
            self.root_view(lcd);
        } else {
            match TaskScreenState::get(self.view_flag) {
                TaskScreenState::Root => {
                    if buttons.f0.state == RELEASED {
                        self.idx = if self.idx == 0 { 3 } else { self.idx - 1 };
                    } else if buttons.f2.state == RELEASED {
                        self.idx = (self.idx + 1) % 4;
                    }
                    match self.idx {
                        1 => {
                            if buttons.f1.state == RELEASED {
                                self.view_flag = TaskScreenState::Categories.val();
                                self.idx = 0;
                            }
                        }
                        2 => {
                            if buttons.f1.state == RELEASED {
                                self.view_flag = TaskScreenState::Overdue.val();
                                self.idx = 0;
                            }
                        }
                        _ => {
                            if buttons.f1.state == RELEASED {
                                self.view_flag = TaskScreenState::AllTasks.val();
                                self.idx = 0;
                            }
                        }
                    }
                }
                TaskScreenState::Categories => {
                    let categories = self.todo.get_categories();
                    if buttons.f1.state == RELEASED {
                        self.cur_category = Some(categories[self.idx].clone());
                        self.idx = 0;
                        self.view_flag = TaskScreenState::CategoryTasks.val();
                    } else {
                        if buttons.f0.state == RELEASED {
                            self.idx = if self.idx == 0 {
                                categories.len() - 1
                            } else {
                                self.idx - 1
                            };
                        } else if buttons.f2.state == RELEASED {
                            self.idx = (self.idx + 1) % categories.len();
                        }
                    }
                }
                TaskScreenState::CategoryTasks => {
                    match self.todo.get_category(self.cur_category.clone()) {
                        Some(tasks) => {
                            if buttons.f1.state == RELEASED {
                                self.cur_category = None;
                                self.cur_id = Some(tasks[self.idx].get_id());
                                self.idx = 0;
                                self.view_flag = TaskScreenState::TaskInfo.val();
                            } else if buttons.f0.state == RELEASED {
                                self.idx = if self.idx == 0 {
                                    tasks.len() - 1
                                } else {
                                    self.idx - 1
                                };
                            } else if buttons.f2.state == RELEASED {
                                self.idx = (self.idx + 1) % tasks.len()
                            }
                        }
                        None => self.first_load(lcd), // This category disappeared somehow, go back to root
                    }
                }
                TaskScreenState::AllTasks => {
                    let tasks = self.todo.get_all_tasks();
                    if buttons.f1.state == RELEASED {
                        self.cur_category = None;
                        self.cur_id = Some(tasks[self.idx].get_id());
                        self.idx = 0;
                        self.view_flag = TaskScreenState::TaskInfo.val();
                    } else if buttons.f0.state == RELEASED {
                        self.idx = if self.idx == 0 {
                            tasks.len() - 1
                        } else {
                            self.idx - 1
                        };
                    } else if buttons.f2.state == RELEASED {
                        self.idx = (self.idx + 1) % tasks.len()
                    }
                }
                TaskScreenState::Overdue => {
                    let tasks = self.todo.get_overdue();
                    if buttons.f1.state == RELEASED {
                        self.cur_category = None;
                        self.cur_id = Some(tasks[self.idx].get_id());
                        self.idx = 0;
                        self.view_flag = TaskScreenState::TaskInfo.val();
                    } else if buttons.f0.state == RELEASED {
                        self.idx = if self.idx == 0 {
                            tasks.len() - 1
                        } else {
                            self.idx - 1
                        };
                    } else if buttons.f2.state == RELEASED {
                        self.idx = (self.idx + 1) % tasks.len()
                    }
                }
                TaskScreenState::TaskInfo => {}
            }

            match TaskScreenState::get(self.view_flag) {
                TaskScreenState::Root => match self.idx {
                    0 => {
                        lcd.clear_jobs();
                        self.root_view(lcd);
                    }
                    1 => {
                        lcd.clear_jobs();
                        lcd.add_job(Job::new("Categories", 0, None));
                        lcd.add_job(Job::empty(1));
                    }
                    2 => {
                        lcd.clear_jobs();
                        lcd.add_job(Job::new("Overdue", 0, None));
                        lcd.add_job(Job::empty(1));
                    }
                    _ => {
                        lcd.clear_jobs();
                        lcd.add_job(Job::new("All Tasks", 0, None));
                        lcd.add_job(Job::empty(1));
                    }
                },
                TaskScreenState::Categories => {
                    let categories = self.todo.get_categories();
                    lcd.clear_jobs();
                    lcd.add_job(Job::new(
                        categories[self.idx].as_str(),
                        0,
                        Some(Duration::from_millis(250)),
                    ));
                    let completion_status = self
                        .todo
                        .get_category_completion(Option::from(categories[self.idx].clone()));
                    lcd.add_job(Job::new(
                        format!("Completion {}/{}", completion_status.0, completion_status.1)
                            .as_str(),
                        0,
                        None,
                    ));
                }
                TaskScreenState::CategoryTasks => {
                    match self.todo.get_category(self.cur_category.clone()) {
                        Some(tasks) => {
                            lcd.clear_jobs();
                            lcd.add_job(Job::new(
                                tasks[self.idx].get_name().as_str(),
                                0,
                                Some(Duration::from_millis(250)),
                            ));
                            lcd.add_job(Job::new(
                                tasks[self.idx].get_desc().as_str(),
                                1,
                                Some(Duration::from_millis(250)),
                            ));
                        }
                        None => self.first_load(lcd),
                    }
                }
                TaskScreenState::AllTasks => {
                    let tasks = self.todo.get_all_tasks();
                    lcd.clear_jobs();
                    lcd.add_job(Job::new(
                        tasks[self.idx].get_name().as_str(),
                        0,
                        Some(Duration::from_millis(250)),
                    ));
                    lcd.add_job(Job::new(
                        tasks[self.idx].get_desc().as_str(),
                        1,
                        Some(Duration::from_millis(250)),
                    ));
                }
                TaskScreenState::Overdue => {
                    let tasks = self.todo.get_overdue();
                    lcd.clear_jobs();
                    lcd.add_job(Job::new(
                        tasks[self.idx].get_name().as_str(),
                        0,
                        Some(Duration::from_millis(250)),
                    ));
                    lcd.add_job(Job::new(
                        tasks[self.idx].get_desc().as_str(),
                        1,
                        Some(Duration::from_millis(250)),
                    ));
                }
                TaskScreenState::TaskInfo => {
                    lcd.clear_jobs();
                    lcd.add_job(Job::new(
                        "Feature not implemented, go back to root",
                        0,
                        Some(Duration::from_millis(250)),
                    ));
                    lcd.add_job(Job::empty(1));
                }
            }
        }
    }
}
