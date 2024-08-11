# e-NOC Scraper

## Installation

* Install rust `https://www.rust-lang.org/tools/install`

* Setup environment variables

    - `ENOC_QUERY_URL` query url
    - `ENOC_AUTHORIZATION_TOKEN` auth token
    - `SPREADSHEET_ID` spreadsheet id
    - `SERVICE_ACCOUNT_JSON` google sheet api key

* cargo run

## Notes

For automated interval runs, need to setup cron job.

Can be setup in windows, linux, or macos. Ideally we run this in server like digital ocean so it runs 24/7
