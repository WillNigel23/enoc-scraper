use anyhow::Result;

use dotenv::dotenv;
use std::{env, str::EncodeUtf16};

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use serde_sheets::{get_sheets, service_account_from_env};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct AlarmItem {
    #[serde(rename = "Alarmmappings.alias")]
    alarm_name: Option<String>,
    #[serde(rename = "Messagesalert.name")]
    severity: Option<String>,
    #[serde(rename = "Alarmmappings.alarmtype")]
    alarm_category: Option<String>,
    #[serde(rename = "Messagesalert.siteId")]
    edgepoint_site_id: Option<String>,
    #[serde(rename = "Site.oldsiteid")]
    old_site_id: Option<String>,
    #[serde(rename = "HardwareMappings.hardwarealias")]
    hardware: Option<String>,
    #[serde(rename = "Site.sitename")]
    site_names: Option<String>,
    #[serde(rename = "Region.regionname")]
    area: Option<String>,
    #[serde(rename = "District.districtName")]
    province: Option<String>,
    #[serde(rename = "TelenorInventoryNwd.criticality")]
    site_category: Option<String>,
    #[serde(rename = "Messagesalert.ariseTime")]
    occurence_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Alarm {
    data: Vec<AlarmItem>,
}

#[derive(Debug, Deserialize)]
struct EnocResponse {
    results: Vec<Alarm>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let enoc_query_url = env::var("ENOC_QUERY_URL").expect("ENOC_QUERY_URL not found in .env");
    let enoc_authorization_token = env::var("ENOC_AUTHORIZATION_TOKEN").expect("ENOC_AUTHORIZATION_TOKEN not found in .env");

    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", enoc_authorization_token))?);

    let response = client
        .get(enoc_query_url)
        .headers(headers)
        .send()
        .await?;

    if response.status().is_success() {
        let enoc_response: EnocResponse = response.json().await?;
        let first_result = enoc_response.results.first().expect("Expected at least one result");
        let result_data = &first_result.data;
        // let results_slice: &[AlarmItem] = std::slice::from_ref(result_data);

        let spreadsheet_id = env::var("SPREADSHEET_ID").expect("SPREADSHEET_ID not found in .env");
        let service_account = service_account_from_env().unwrap();
        let mut sheets = get_sheets(service_account, Some("token_cache.json"))
            .await
            .unwrap();

        serde_sheets::write_page(&mut sheets, &spreadsheet_id, "Sheet1", &result_data)
            .await
            .unwrap();
    } else {
        println!("Error: {:?}", response.status());
    }

    Ok(())
}
