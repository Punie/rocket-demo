#![feature(proc_macro_hygiene, decl_macro, never_type)]
#![allow(dead_code)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;

mod schema;
mod task;

use diesel::SqliteConnection;
use rocket::{
    http::{RawStr, Status},
    request::{Form, FromParam, FromRequest, Outcome, Request},
    response::{
        status::{Created, Custom},
        Redirect,
    },
    Rocket,
};
use rocket_contrib::{
    json::{Json, JsonValue},
    templates::Template,
};
use serde::Serialize;
use task::{Task, Todo};

struct Age(i32);

impl<'r> FromParam<'r> for Age {
    type Error = &'r RawStr;

    fn from_param(param: &'r RawStr) -> Result<Self, Self::Error> {
        let value = i32::from_param(param)?;

        if value >= 18 {
            Ok(Age(value))
        } else {
            Err(param)
        }
    }
}

struct User {
    token: String,
}

impl User {
    fn is_admin(&self) -> bool {
        self.token.contains("admin")
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = !;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        if let Some(auth) = request.headers().get_one("Authorization") {
            let token = auth.replace("Bearer ", "");
            Outcome::Success(User { token })
        } else {
            Outcome::Forward(())
        }
    }
}

struct Admin {
    user: User,
}

impl<'a, 'r> FromRequest<'a, 'r> for Admin {
    type Error = !;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let user = request.guard::<User>()?;

        if user.is_admin() {
            Outcome::Success(Admin { user })
        } else {
            Outcome::Forward(())
        }
    }
}

#[derive(FromForm)]
struct Auth {
    email: String,
    password: String,
}

#[database("tasks")]
struct DbConn(SqliteConnection);

#[derive(Serialize)]
struct ApiError {
    code: usize,
    name: String,
    message: String,
}

/// Hello world
#[get("/")]
fn hello() -> &'static str {
    "Hello world!"
}

/// Route parameters
#[get("/hello/<name>/<age>")]
fn person(name: String, age: i32) -> String {
    format!("Hello, {} year old named {}!", age, name)
}

/// Custom param validation
#[get("/hello/<age>")]
fn adult(age: Age) -> String {
    format!("At {}, you are old enough: welcome!", age.0)
}

/// Ranking
#[get("/hello/<age>", rank = 2)]
fn child(age: i32) -> String {
    format!(
        "Sorry, {} is too young to enter, come back in a few years.",
        age
    )
}

/// Request guards
#[get("/admin")]
fn admin_dashboard(_admin: Admin) -> String {
    String::from("Welcome, administrator!")
}

#[get("/admin", rank = 2)]
fn user_dashboard(_user: User) -> String {
    String::from("Welcome, simple user!")
}

#[get("/admin", rank = 3)]
fn unauthenticated_user() -> Redirect {
    Redirect::to(uri!(login_page))
}

/// Tera Templates
#[get("/login")]
fn login_page() -> Template {
    Template::render("login", json!({}))
}

#[post("/login", data = "<auth>")]
fn login(auth: Form<Auth>) -> JsonValue {
    if auth.password == "admin" {
        json!({ "token": "admin" })
    } else {
        json!({ "token": "hugo" })
    }
}

/// CRUD (DB access, JSON, Responders)
#[get("/todos")]
fn get_tasks(conn: DbConn) -> Json<Vec<Task>> {
    Json(Task::all(&conn))
}

#[get("/todos/<id>")]
fn get_task(id: i32, conn: DbConn) -> Option<Json<Task>> {
    Task::get_one(id, &conn).map(Json)
}

#[put("/todos/<id>")]
fn toggle_task(id: i32, conn: DbConn) -> Option<Json<Task>> {
    Task::toggle_with_id(id, &conn).map(Json)
}

#[post("/todos", format = "json", data = "<todo>")]
fn create_task(todo: Json<Todo>, conn: DbConn) -> Option<Created<Json<Task>>> {
    Task::insert(todo.into_inner(), &conn).map(|task| {
        Created(
            uri!("/api", get_task: id = task.id).to_string(),
            Some(Json(task)),
        )
    })
}

#[delete("/todos/<id>")]
fn delete_task(id: i32, conn: DbConn) -> Option<Custom<()>> {
    if Task::delete_with_id(id, &conn) {
        Some(Custom(Status::NoContent, ()))
    } else {
        None
    }
}

/// Error catchers
#[catch(404)]
fn not_found(_: &Request) -> Json<ApiError> {
    Json(ApiError {
        code: 404,
        name: String::from("Not Found"),
        message: String::from("Four, oh four!"),
    })
}

#[catch(422)]
fn unprocessable_entity(_: &Request) -> Json<ApiError> {
    Json(ApiError {
        code: 422,
        name: String::from("Unprocessable Entity"),
        message: String::from(
            "The request was well-formed but was unable to be followed due to semantic errors.",
        ),
    })
}

/// Rocket instance
fn ignite_rocket() -> Rocket {
    rocket::ignite()
        .attach(DbConn::fairing())
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                hello,
                person,
                adult,
                child,
                admin_dashboard,
                user_dashboard,
                unauthenticated_user,
                login_page,
                login
            ],
        )
        .mount(
            "/api",
            routes![get_tasks, get_task, create_task, toggle_task, delete_task],
        )
        .register(catchers![not_found, unprocessable_entity])
}

fn main() {
    ignite_rocket().launch();
}

#[cfg(test)]
mod tests {
    use super::ignite_rocket;
    use rocket::{
        http::{Header, Status},
        local::Client,
    };

    #[test]
    fn hello() {
        let rocket = ignite_rocket();
        let client = Client::new(rocket).unwrap();

        let mut response = client.get("/").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.body_string(), Some(String::from("Hello world!")));
    }

    #[test]
    fn person() {
        let rocket = ignite_rocket();
        let client = Client::new(rocket).unwrap();

        let mut response = client.get("/hello/hugo/30").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.body_string(),
            Some(String::from("Hello, 30 year old named hugo!"))
        );
    }

    #[test]
    fn age() {
        let rocket = ignite_rocket();
        let client = Client::new(rocket).unwrap();

        let mut response = client.get("/hello/30").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.body_string(),
            Some(String::from("At 30, you are old enough: welcome!"))
        );

        let mut response = client.get("/hello/15").dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.body_string(),
            Some(String::from(
                "Sorry, 15 is too young to enter, come back in a few years."
            ))
        );
    }

    #[test]
    fn auth() {
        let rocket = ignite_rocket();
        let client = Client::new(rocket).unwrap();

        let mut response = client
            .get("/admin")
            .header(Header::new("Authorization", "Bearer admin"))
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.body_string(),
            Some(String::from("Welcome, administrator!"))
        );

        let mut response = client
            .get("/admin")
            .header(Header::new("Authorization", "Bearer user"))
            .dispatch();

        assert_eq!(response.status(), Status::Ok);
        assert_eq!(
            response.body_string(),
            Some(String::from("Welcome, simple user!"))
        );

        let response = client.get("/admin").dispatch();

        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(response.headers().get_one("Location"), Some("/login"));
    }
}
