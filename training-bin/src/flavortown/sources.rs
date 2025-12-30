use serde::Deserialize;

use crate::network::{Pagination, Pagintated};

#[derive(Deserialize, Clone)]
pub struct Devlogs {
    devlogs: Vec<Devlog>,
    pagination: Pagination,
}

#[derive(Deserialize, Clone)]
pub struct Devlog {
    pub body: String,
}

#[derive(Deserialize, Clone)]
pub struct Projects {
    projects: Vec<Project>,
    pagination: Pagination,
}

#[derive(Deserialize, Clone)]
pub struct Project {
    pub description: String,
}

impl Pagintated for Devlogs {
    const ROOT: &str = "https://flavortown.hackclub.com/api/v1/devlogs";
    type Data = Devlog;

    fn page(self) -> Vec<Devlog> {
        self.devlogs
    }

    fn pagination(&self) -> &Pagination {
        &self.pagination
    }
}

impl Pagintated for Projects {
    const ROOT: &str = "https://flavortown.hackclub.com/api/v1/projects";
    type Data = Project;

    fn page(self) -> Vec<Project> {
        self.projects
    }

    fn pagination(&self) -> &Pagination {
        &self.pagination
    }
}
