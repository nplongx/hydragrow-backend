use crate::db::DbResult;
use crate::models::sensor::SensorData;
use chrono::{DateTime, Utc};
use futures_util::stream;
use influxdb2::{
    Client,
    models::{DataPoint, Query},
};
use tracing::{error, instrument};

#[instrument(skip(client, data))]
pub async fn write_sensor_data(client: &Client, bucket: &str, data: &SensorData) -> DbResult<()> {
    // Build DataPoint theo schema measurement: sensor_data
    let point = DataPoint::builder("sensor_data")
        .tag("device_id", &data.device_id)
        .field("ec_value", data.ec_value)
        .field("ph_value", data.ph_value)
        .field("temp_value", data.temp_value)
        .field("water_level", data.water_level)
        .field("pump_status", data.pump_status.as_str())
        .timestamp(data.timestamp.timestamp_nanos_opt().unwrap_or(0))
        .build()
        .map_err(|e| crate::db::DbError::ParseError(e.to_string()))?;

    // Dùng stream::iter vì client.write yêu cầu một Stream
    client.write(bucket, stream::iter(vec![point])).await?;

    Ok(())
}

#[instrument(skip(client))]
pub async fn get_sensor_history(
    client: &Client,
    bucket: &str,
    device_id: &str,
    start_rfc3339: &str,
    end_rfc3339: &str,
    limit: usize,
) -> DbResult<Vec<SensorData>> {
    // Dùng hàm time(v: "...") của Flux để parse chuỗi string thành định dạng thời gian chuẩn
    let flux_query = format!(
        r#"
        from(bucket: "{}")
            |> range(start: time(v: "{}"), stop: time(v: "{}"))
            |> filter(fn: (r) => r["_measurement"] == "sensor_data" and r["device_id"] == "{}")
            |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
            |> sort(columns: ["_time"], desc: true)
            |> limit(n: {})
        "#,
        bucket, start_rfc3339, end_rfc3339, device_id, limit
    );

    let query_obj = Query::new(flux_query);

    // Gọi client query. Kết quả trả về là một danh sách các "Table"
    let tables = client.query::<SensorData>(Some(query_obj)).await?;

    let mut history = Vec::new();

    for record in tables {
        history.push(record);
    }

    Ok(history)
}
