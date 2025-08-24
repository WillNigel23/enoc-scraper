use anyhow::Result;
use dotenv::dotenv;
use std::env;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::Error;
use serde::{Deserialize, Serialize};
use serde_sheets::{get_sheets, service_account_from_env};
use chrono::{Local, Utc, Duration as ChronoDuration};
use tokio::time::{self, Duration};
use url::Url;
use serde_json::Value;
use serde_urlencoded;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct HistoricAlarmItem {
    #[serde(rename = "Messagesalerthistory.siteid")]
    edgepoint_site_id: Option<String>,
    #[serde(rename = "Site.oldsiteid")]
    old_site_id: Option<String>,
    #[serde(rename = "Site.sitename")]
    site_names: Option<String>,
    #[serde(rename = "Alarmmappings.alias")]
    alarm_name: Option<String>,
    #[serde(rename = "Alarmmappings.alarmtype")]
    alarm_category: Option<String>,
    #[serde(rename = "Region.regionname")]
    area: Option<String>,
    #[serde(rename = "District.districtName")]
    province: Option<String>,
    #[serde(rename = "Messagesalerthistory.opentime")]
    occurence_time: Option<String>,
    #[serde(rename = "Messagesalerthistory.closetime")]
    end_time: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ActiveAlarmItem {
    #[serde(rename = "Messagesalert.siteId")]
    edgepoint_site_id: Option<String>,
    #[serde(rename = "Site.oldsiteid")]
    old_site_id: Option<String>,
    #[serde(rename = "Site.sitename")]
    site_names: Option<String>,
    #[serde(rename = "Alarmmappings.alias")]
    alarm_name: Option<String>,
    #[serde(rename = "Alarmmappings.alarmtype")]
    alarm_category: Option<String>,
    #[serde(rename = "Region.regionname")]
    area: Option<String>,
    #[serde(rename = "District.districtName")]
    province: Option<String>,
    #[serde(rename = "Messagesalert.ariseTime")]
    occurence_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Alarm<T> {
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct EnocResponse<T> {
    results: Vec<Alarm<T>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AuthToken {
    token: String,
}

fn last_60_days_unix() -> (i64, i64) {
    let now = Utc::now().timestamp();
    let sixty_days_ago = now - ChronoDuration::days(60).num_seconds();
    (sixty_days_ago, now)
}

async fn build_query_url(base_url: &str) -> Result<String> {
    let mut parsed_url = Url::parse(base_url)?;
    let mut query_pairs: Vec<(String, String)> = parsed_url
        .query_pairs()
        .into_owned()
        .collect();

    if let Some((_, query_json)) = query_pairs.iter_mut().find(|(k, _)| k == "query") {
        let mut query: Value = serde_json::from_str(query_json)?;

        let (start, end) = last_60_days_unix();

        if let Some(filters) = query.get_mut("filters").and_then(|f| f.as_array_mut()) {
            filters.push(serde_json::json!({
                "member": "Messagesalerthistory.opentimeunix",
                "operator": "gte",
                "values": [start]
            }));
            filters.push(serde_json::json!({
                "member": "Messagesalerthistory.opentimeunix",
                "operator": "lte",
                "values": [end]
            }));
        }

        *query_json = serde_json::to_string(&query)?;
    }

    parsed_url.set_query(Some(&serde_urlencoded::to_string(&query_pairs)?));

    Ok(parsed_url.to_string()) // use to_string instead of deprecated into_string
}

async fn auth_token(query_url: &str) -> Result<String, Error> {
    let response = reqwest::get(query_url).await?;
    let auth_response: AuthToken = response.json().await?;
    Ok(auth_response.token)
}

async fn update_historic_sheet() -> Result<()> {
    let base_query_url = env::var("ENOC_QUERY_URL").expect("ENOC_QUERY_URL not found in .env");
    let enoc_query_url = build_query_url(&base_query_url).await?;
    let enoc_authorization_token_url = env::var("ENOC_AUTHORIZATION_TOKEN").expect("ENOC_AUTHORIZATION_TOKEN not found in .env");
    let enoc_authorization_token = match auth_token(&enoc_authorization_token_url).await {
        Ok(val) => val,
        Err(e) => {
            println!("Error fetching auth token: {:?}", e);
            return Ok(());
        }
    };

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", enoc_authorization_token))?);

    let response = client.get(enoc_query_url).headers(headers).send().await?;

    if response.status().is_success() {
        let enoc_response: EnocResponse<HistoricAlarmItem> = response.json().await?;
        let first_result = enoc_response.results.first().expect("Expected at least one result");
        let result_data = &first_result.data;

        let spreadsheet_id = env::var("SPREADSHEET_ID").expect("SPREADSHEET_ID not found in .env");
        let service_account = match service_account_from_env() {
            Ok(val) => val,
            Err(e) => {
                println!("Error loading service account: {:?}", e);
                return Ok(());
            }
        };
        let mut sheets = match get_sheets(service_account, Some("token_cache.json")).await {
            Ok(val) => val,
            Err(e) => {
                println!("Error initializing sheets: {:?}", e);
                return Ok(());
            }
        };

        if let Err(e) = serde_sheets::write_page(&mut sheets, &spreadsheet_id, "Historic", &result_data).await {
            println!("Error writing Historic sheet: {:?}", e);
            return Ok(());
        }

        let current_time = Local::now();
        println!("Successfully updated Historic sheet at {:?}", current_time);
    } else {
        println!("Error: {:?}", response.status());
    }

    Ok(())
}

async fn update_active_sheet() -> Result<()> {
    let enoc_query_url = env::var("ENOC_QUERY_URL_2").expect("ENOC_QUERY_URL_2 not found in .env");
    let enoc_authorization_token_url = env::var("ENOC_AUTHORIZATION_TOKEN").expect("ENOC_AUTHORIZATION_TOKEN not found in .env");
    let enoc_authorization_token = match auth_token(&enoc_authorization_token_url).await {
        Ok(val) => val,
        Err(e) => {
            println!("Error fetching auth token: {:?}", e);
            return Ok(());
        }
    };

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", enoc_authorization_token))?);

    let response = client.get(enoc_query_url).headers(headers).send().await?;

    if response.status().is_success() {
        let enoc_response: EnocResponse<ActiveAlarmItem> = response.json().await?;
        let first_result = enoc_response.results.first().expect("Expected at least one result");
        let result_data = &first_result.data;

        let spreadsheet_id = env::var("SPREADSHEET_ID").expect("SPREADSHEET_ID not found in .env");
        let service_account = match service_account_from_env() {
            Ok(val) => val,
            Err(e) => {
                println!("Error loading service account: {:?}", e);
                return Ok(());
            }
        };
        let mut sheets = match get_sheets(service_account, Some("token_cache.json")).await {
            Ok(val) => val,
            Err(e) => {
                println!("Error initializing sheets: {:?}", e);
                return Ok(());
            }
        };

        if let Err(e) = serde_sheets::write_page(&mut sheets, &spreadsheet_id, "Active", &result_data).await {
            println!("Error writing Active sheet: {:?}", e);
            return Ok(());
        }

        let current_time = Local::now();
        println!("Successfully updated Active sheet at {:?}", current_time);
    } else {
        println!("Error: {:?}", response.status());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let mut interval = time::interval(Duration::from_secs(300));

    loop {
        interval.tick().await;
        println!("Running scheduled update...");
        let _ = update_historic_sheet().await;
        let _ = update_active_sheet().await;
    }
}
