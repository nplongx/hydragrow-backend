use anyhow::{Context, Result};
use chrono::Utc;
use influxdb::{Client, ReadQuery, Timestamp, WriteQuery};
use serde_json::Value;
use tracing::{info, instrument};

use crate::models::sensor::SensorData;

#[instrument(skip(client, data))]
pub async fn write_sensor_data(client: &Client, _bucket: &str, data: &SensorData) -> Result<()> {
    let pump_status_json = serde_json::to_string(&data.pump_status)
        .context("Failed to serialize pump_status to JSON")?;

    // Dùng WriteQuery::new() thay vì gọi .into_query() từ Timestamp
    let write_query = WriteQuery::new(Timestamp::try_from(Utc::now()).unwrap(), "sensor_data")
        .add_tag("device_id", data.device_id.clone())
        .add_field("ec_value", data.ec_value)
        .add_field("ph_value", data.ph_value)
        .add_field("temp_value", data.temp_value)
        .add_field("water_level", data.water_level)
        .add_field("pump_status", pump_status_json);

    client
        .query(write_query)
        .await
        .context("Failed to write to InfluxDB")?;

    Ok(())
}

#[instrument(skip(client))]
pub async fn get_latest_sensor_data(
    client: &Client,
    _bucket: &str,
    device_id: &str,
) -> Result<SensorData> {
    let query_str = format!(
        "SELECT * FROM sensor_data WHERE device_id = '{}' ORDER BY time DESC LIMIT 1",
        device_id
    );
    let read_query = ReadQuery::new(query_str);

    // Ở crate influxdb v0.8.0, truy vấn sẽ trả về 1 mảng JSON dưới dạng String
    let query_result_str = client
        .query(read_query)
        .await
        .context("InfluxQL query failed")?;

    // Sử dụng serde_json để bóc tách cấu trúc dữ liệu của InfluxDB v1
    let parsed: Value = serde_json::from_str(&query_result_str)
        .context("Failed to parse InfluxDB JSON response")?;

    // Cấu trúc InfluxDB trả về: parsed["results"][0]["series"][0]
    let series_arr = parsed["results"][0]["series"].as_array();

    if let Some(series) = series_arr {
        if let Some(first_series) = series.first() {
            let columns = first_series["columns"]
                .as_array()
                .context("Missing columns")?;
            let values = first_series["values"][0]
                .as_array()
                .context("Missing values")?;

            // Helper function để lấy dữ liệu map theo tên cột
            let get_f64 = |col_name: &str| -> f64 {
                columns
                    .iter()
                    .position(|c| c.as_str() == Some(col_name))
                    .and_then(|idx| values[idx].as_f64())
                    .unwrap_or(0.0)
            };

            let get_str = |col_name: &str| -> String {
                columns
                    .iter()
                    .position(|c| c.as_str() == Some(col_name))
                    .and_then(|idx| values[idx].as_str())
                    .unwrap_or("")
                    .to_string()
            };

            let pump_status_str = get_str("pump_status");
            let pump_status = serde_json::from_str(&pump_status_str).unwrap_or_default();

            let sensor_data = SensorData {
                device_id: get_str("device_id"),
                ec_value: get_f64("ec_value"),
                ph_value: get_f64("ph_value"),
                temp_value: get_f64("temp_value"),
                water_level: get_f64("water_level"),
                pump_status,
                time: get_str("time"),
            };

            info!("Latest sensor: {:?}", sensor_data);
            return Ok(sensor_data);
        }
    }

    Err(anyhow::anyhow!(
        "No sensor data found for device: {}",
        device_id
    ))
}
