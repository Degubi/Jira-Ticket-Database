use std::{fs::File, io::Write};
use base64::{engine::general_purpose, Engine as _};
use regex_lite::Regex;
use rusqlite::{Connection, params};
use serde::Deserialize;

fn main() {
    let db_connection = Connection::open("jira.db").unwrap();

    if let Some("reset") = std::env::args().nth(1).as_ref().map(String::as_str) && read_from_console("Are you sure you want to reset Issue Updates? y/n") == "y" {
        db_connection.execute("DELETE FROM IssueUpdates", []).unwrap();
        std::process::exit(0);
    }

    let config = match File::open("config.json") {
        Ok(f) => serde_json::from_reader::<_, Config>(f).unwrap(),
        Err(e) => panic!("Unable to read config.json, error: {e}")
    };

    let time_regex = Regex::new(r"^(?:(?<days>\d+)d)?\s*(?:(?<hours>\d+)h)?\s*(?:(?<minutes>\d+)m)?$").unwrap();
    let api_response = ureq::get(format!("{}/rest/api/3/search/jql?jql=assignee=currentUser()&fields=id,timetracking&maxResults=5000", config.domain))
         .header("Accept", "application/json")
         .header("Authorization", &format!("Basic {}", general_purpose::STANDARD.encode(format!("{}:{}", config.email, config.key))))
         .call();

    match api_response {
        Ok(mut k) => match serde_json::from_str::<JiraResponse>(&k.body_mut().read_to_string().unwrap()) {
            Ok(response) => response.issues.iter().for_each(|jira_issue| process_jira_issue(jira_issue, &db_connection, &time_regex)),
            Err(e) => println!("Was unable to parse Jira response: {e}")
        }
        Err(e) => println!("Jira request error: {e}")
    }

    println!("Updated tickets:");

    db_connection.prepare("SELECT key, SUM(hoursChange), SUM(minutesChange) FROM IssueUpdates GROUP BY key").unwrap()
                 .query_map([], |row| Ok(DatabaseIssue {
                    key: row.get(0).unwrap(),
                    hours: row.get(1).unwrap(),
                    minutes: row.get(2).unwrap()
                 })).unwrap()
                 .map(Result::unwrap)
                 .for_each(|k| println!("{}: {}h {}m", k.key, k.hours, k.minutes));

    let total_update_minutes = db_connection.prepare("SELECT coalesce(SUM(hoursChange * 60 + minutesChange), 0) FROM IssueUpdates").unwrap()
                                            .query_one([], |row| Ok(row.get::<_, i32>(0))).unwrap()
                                            .unwrap();

    let update_hours = total_update_minutes / 60;
    let update_minutes = total_update_minutes % 60;

    println!("Total time: {}h {}m", update_hours, update_minutes);
}

fn process_jira_issue(jira_issue: &JiraIssue, db_connection: &Connection, time_regex: &Regex) {
    let db_issue_stats = db_connection.prepare_cached("SELECT key, hours, minutes FROM Issues WHERE key = ?1").unwrap()
                                      .query_one([ &jira_issue.key ], |row| Ok(DatabaseIssue {
                                          key: row.get(0).unwrap(),
                                          hours: row.get(1).unwrap(),
                                          minutes: row.get(2).unwrap()
                                      })).ok();

    match db_issue_stats {
        Some(DatabaseIssue { key: issue_key, hours: db_issue_hours, minutes: db_issue_minutes }) => {
            let jira_time_parts = parse_time(jira_issue.fields.timetracking.timeSpent.as_ref().map(String::as_str).unwrap_or(""), time_regex);

            match (jira_time_parts, (db_issue_hours, db_issue_minutes)) {
                ((0, 0), (0, 0)) => {},
                ((0, 0), (_, _)) => panic!("This should never happen!"),
                (jira_time, (0, 0)) => handle_issue_with_newly_set_time(&issue_key, jira_time, db_connection),
                (jira_time, db_time) if jira_time == db_time => {},
                (jira_time, db_time) => handle_issue_with_time_change(&issue_key, jira_time, db_time, db_connection)
            }
        },
        None => handle_issue_not_in_db(jira_issue, db_connection, time_regex)
    }
}

fn handle_issue_with_time_change(issue_key: &str, (jira_hours, jira_minutes): (i32, i32), (db_hours, db_minutes): (i32, i32), db_connection: &Connection) {
    if read_from_console(&format!("Issue '{issue_key}' previously had {}h {}m time in db, jira has {}h {}m. Update it in DB? (y = yes, n = no)", db_hours, db_minutes, jira_hours, jira_minutes)) == "y" {
        db_connection.execute("UPDATE Issues SET hours = ?2, minutes = ?3 WHERE key = ?1", params![ issue_key, jira_hours, jira_minutes ]).unwrap();

        let (hour_updates, minute_updates) = db_connection.prepare_cached("SELECT coalesce(SUM(hoursChange), 0), coalesce(SUM(minutesChange), 0) FROM IssueUpdates WHERE key = ?1").unwrap()
                                                          .query_one([ issue_key ], |row| Ok((row.get::<_, i32>(0).unwrap(), row.get::<_, i32>(1).unwrap()))).unwrap();

        let old_total_minutes = db_hours * 60 + db_minutes;
        let new_total_minutes = jira_hours * 60 + jira_minutes;
        let updates_total_minutes = hour_updates * 60 + minute_updates;

        let actual_time_diff = new_total_minutes - updates_total_minutes - old_total_minutes;
        let updated_hours = actual_time_diff / 60;
        let updated_minutes = actual_time_diff % 60;

        db_connection.execute("INSERT INTO IssueUpdates(key, hoursChange, minutesChange) VALUES(?1, ?2, ?3)", params![ &issue_key, updated_hours, updated_minutes ]).unwrap();
    }
}

fn handle_issue_with_newly_set_time(issue_key: &str, (jira_minutes, jira_hours): (i32, i32), db_connection: &Connection) {
    if read_from_console(&format!("Issue '{issue_key}' had no time in db, received {}h {}m. Update it in DB? (y = yes, n = no)", jira_hours, jira_minutes)) == "y" {
        let params = params![ issue_key, jira_hours, jira_minutes ];

        db_connection.execute("UPDATE Issues SET hours = ?2, minutes = ?3 WHERE key = ?1", params).unwrap();
        db_connection.execute("INSERT INTO IssueUpdates(key, hoursChange, minutesChange) VALUES(?1, ?2, ?3)", params).unwrap();
    }
}

fn handle_issue_not_in_db(issue: &JiraIssue, db_connection: &Connection, time_regex: &Regex) {
    if read_from_console(&format!("Found new issue: {}, add it to DB? (y = yes, n = no)", issue.key)) == "y" {
        let (hours, minutes) = parse_time(issue.fields.timetracking.timeSpent.as_ref().map(String::as_str).unwrap_or(""), time_regex);
        let params = params![ &issue.key, hours, minutes ];

        db_connection.execute("INSERT INTO Issues(key, hours, minutes) VALUES(?1, ?2, ?3)", params).unwrap();
        db_connection.execute("INSERT INTO IssueUpdates(key, hoursChange, minutesChange) VALUES(?1, ?2, ?3)", params).unwrap();
    }
}

fn parse_time(time_str: &str, time_regex: &Regex) -> (i32, i32) {
    match time_regex.captures(time_str) {
        Some(k) => (
            k.name("days").map(|l| l.as_str().parse::<i32>().unwrap()).unwrap_or(0) * 8 + k.name("hours").map(|l| l.as_str().parse::<i32>().unwrap()).unwrap_or(0),
            k.name("minutes").map(|l| l.as_str().parse::<i32>().unwrap()).unwrap_or(0)
        ),
        None => (0, 0)
    }
}

#[allow(unused_must_use)]
fn read_from_console(prompt: &str) -> String {
    print!("{prompt} ");
    std::io::stdout().flush();
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer);
    buffer.trim().to_string()
}

#[derive(Deserialize)]
struct JiraResponse {
    issues: Vec<JiraIssue>
}

#[derive(Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraIssueFields
}

#[derive(Deserialize)]
struct JiraIssueFields {
    timetracking: JiraTimeTracking
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct JiraTimeTracking {
    timeSpent: Option<String>
}

struct DatabaseIssue {
    key: String,
    hours: i32,
    minutes: i32
}

#[derive(Deserialize)]
struct Config {
    email: String,
    key: String,
    domain: String
}
