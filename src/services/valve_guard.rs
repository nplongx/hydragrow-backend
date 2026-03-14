// use crate::models::sensor::{DeviceState, PumpStatus, ValveCommandReq, ValveState};
// use thiserror::Error;
//
// #[derive(Error, Debug)]
// pub enum ValveSafetyError {
//     #[error(
//         "Conflict: Không thể mở {requested} vì {current_open} đang mở. Phải đóng {current_open} trước!"
//     )]
//     ConflictBothValves {
//         requested: String,
//         current_open: String,
//     },
//     #[error("Safety Rule Violation: Lệnh yêu cầu mở van nhưng WATER_PUMP đang tắt.")]
//     WaterPumpOff,
//     #[error("Invalid Valve Target: {0}")]
//     InvalidValve(String),
// }
//
// /// Hàm này sẽ được gọi ở `api/control.rs` TRƯỚC KHI publish MQTT.
// /// current_status được query từ record mới nhất trong InfluxDB.
// pub fn validate_valve_action(
//     req: &ValveCommandReq,
//     current_status: &PumpStatus,
// ) -> Result<(), ValveSafetyError> {
//     // Nếu là lệnh "closed" thì luôn cho phép chạy (ưu tiên tắt để an toàn)
//     if req.action == ValveState::Closed {
//         return Ok(());
//     }
//
//     // Nếu là lệnh "open", bắt đầu kiểm tra safety rules
//     match req.valve.as_str() {
//         "VAN_IN" => {
//             // 1. Kiểm tra van đối nghịch
//             if current_status.van_out == ValveState::Open {
//                 return Err(ValveSafetyError::ConflictBothValves {
//                     requested: "VAN_IN".to_string(),
//                     current_open: "VAN_OUT".to_string(),
//                 });
//             }
//         }
//         "VAN_OUT" => {
//             // 1. Kiểm tra van đối nghịch
//             if current_status.van_in == ValveState::Open {
//                 return Err(ValveSafetyError::ConflictBothValves {
//                     requested: "VAN_OUT".to_string(),
//                     current_open: "VAN_IN".to_string(),
//                 });
//             }
//         }
//         _ => return Err(ValveSafetyError::InvalidValve(req.valve.clone())),
//     }
//
//     // 2. Rule: Không mở van khi không có lệnh bật WATER_PUMP (Tránh cháy bơm hoặc hỏng van do áp suất)
//     // Lưu ý: Tùy logic phần cứng, nếu bơm bật CÙNG LÚC với van trong 1 payload API thì rule này
//     // có thể cần điều chỉnh. Hiện tại check theo trạng thái đang chạy.
//     if current_status.water_pump == DeviceState::Off {
//         return Err(ValveSafetyError::WaterPumpOff);
//     }
//
//     Ok(())
// }
