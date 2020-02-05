use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::schema::tasks;
use crate::schema::tasks::dsl::{completed as task_completed, tasks as all_tasks};

#[table_name = "tasks"]
#[derive(Serialize, Queryable, Identifiable, Debug)]
pub struct Task {
    pub id: i32,
    pub description: String,
    pub completed: bool,
}

#[table_name = "tasks"]
#[derive(Deserialize, Insertable)]
pub struct Todo {
    pub description: String,
}

impl Task {
    pub fn all(conn: &SqliteConnection) -> Vec<Task> {
        all_tasks
            .order(tasks::id.desc())
            .load::<Task>(conn)
            .unwrap()
    }

    pub fn get_one(id: i32, conn: &SqliteConnection) -> Option<Task> {
        all_tasks.find(id).get_result::<Task>(conn).ok()
    }

    pub fn insert(todo: Todo, conn: &SqliteConnection) -> Option<Task> {
        conn.transaction(|| {
            diesel::insert_into(tasks::table)
                .values(todo)
                .execute(conn)
                .and_then(|_| all_tasks.order(tasks::id.desc()).first::<Task>(conn))
        })
        .ok()
    }

    pub fn toggle_with_id(id: i32, conn: &SqliteConnection) -> Option<Task> {
        conn.transaction(|| {
            all_tasks
                .find(id)
                .get_result::<Task>(conn)
                .and_then(|task| {
                    diesel::update(&task)
                        .set(task_completed.eq(!task.completed))
                        .execute(conn)
                })
                .and_then(|_| all_tasks.find(id).get_result::<Task>(conn))
        })
        .ok()
    }

    pub fn delete_with_id(id: i32, conn: &SqliteConnection) -> bool {
        diesel::delete(all_tasks.find(id))
            .execute(conn)
            .map(|n| n > 0)
            .unwrap_or_default()
    }
}
