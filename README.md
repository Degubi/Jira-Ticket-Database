# Jira-Ticket-Database

Application that keeps my Jira tickets in a DB and tells me how much I've worked...

## Setup
- Create an SQLite database named 'jira.db', table schemas are in 'Jira.sql'.
- Create a config.json file like this:
```jsonc
{
    "email": "JIRA_EMAIL_HERE",
    "key": "JIRA_API_KEY_HERE",  // You can get an API key inside your Jira account settings. (Security -> API Tokens)
    "domain": "JIRA_DOMAIN_HERE" // This is the domain of your Jira account. It looks like this: https://{SOMETHING}.atlassian.net
}
```

## Running
- Run the project with 'cargo run'.
- The application queries the list of tickets, on the first use you have to add all your existing tickets to the database.
- Using the application is really simple (I hope), it will ask the user whether he/she wants to add a ticket or update a ticket while processing them.
- At the end it will always print the list of tickets with time changes and a sum of all your time.
- Resetting changes inside the db is really easy: 'cargo run reset'.
- I MIGHT create prebuilt binaries.