use crate::mcp_core::{McpResult, McpTool};
use super::Tool;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Time Zone Database (IANA timezones with UTC offsets in seconds)
// ============================================================================

struct TimezoneInfo {
    name: &'static str,
    offset_seconds: i32,
    abbreviation: &'static str,
}

const TIMEZONES: &[TimezoneInfo] = &[
    // UTC
    TimezoneInfo { name: "UTC", offset_seconds: 0, abbreviation: "UTC" },

    // Asia
    TimezoneInfo { name: "Asia/Ho_Chi_Minh", offset_seconds: 7 * 3600, abbreviation: "ICT" },
    TimezoneInfo { name: "Asia/Bangkok", offset_seconds: 7 * 3600, abbreviation: "ICT" },
    TimezoneInfo { name: "Asia/Jakarta", offset_seconds: 7 * 3600, abbreviation: "WIB" },
    TimezoneInfo { name: "Asia/Singapore", offset_seconds: 8 * 3600, abbreviation: "SGT" },
    TimezoneInfo { name: "Asia/Hong_Kong", offset_seconds: 8 * 3600, abbreviation: "HKT" },
    TimezoneInfo { name: "Asia/Shanghai", offset_seconds: 8 * 3600, abbreviation: "CST" },
    TimezoneInfo { name: "Asia/Taipei", offset_seconds: 8 * 3600, abbreviation: "CST" },
    TimezoneInfo { name: "Asia/Tokyo", offset_seconds: 9 * 3600, abbreviation: "JST" },
    TimezoneInfo { name: "Asia/Seoul", offset_seconds: 9 * 3600, abbreviation: "KST" },
    TimezoneInfo { name: "Asia/Kolkata", offset_seconds: 5 * 3600 + 1800, abbreviation: "IST" },
    TimezoneInfo { name: "Asia/Dubai", offset_seconds: 4 * 3600, abbreviation: "GST" },

    // Europe
    TimezoneInfo { name: "Europe/London", offset_seconds: 0, abbreviation: "GMT" },
    TimezoneInfo { name: "Europe/Paris", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Berlin", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Amsterdam", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Brussels", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Rome", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Madrid", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Zurich", offset_seconds: 1 * 3600, abbreviation: "CET" },
    TimezoneInfo { name: "Europe/Moscow", offset_seconds: 3 * 3600, abbreviation: "MSK" },

    // Americas
    TimezoneInfo { name: "America/New_York", offset_seconds: -5 * 3600, abbreviation: "EST" },
    TimezoneInfo { name: "America/Chicago", offset_seconds: -6 * 3600, abbreviation: "CST" },
    TimezoneInfo { name: "America/Denver", offset_seconds: -7 * 3600, abbreviation: "MST" },
    TimezoneInfo { name: "America/Los_Angeles", offset_seconds: -8 * 3600, abbreviation: "PST" },
    TimezoneInfo { name: "America/San_Francisco", offset_seconds: -8 * 3600, abbreviation: "PST" },
    TimezoneInfo { name: "America/Toronto", offset_seconds: -5 * 3600, abbreviation: "EST" },
    TimezoneInfo { name: "America/Vancouver", offset_seconds: -8 * 3600, abbreviation: "PST" },
    TimezoneInfo { name: "America/Sao_Paulo", offset_seconds: -3 * 3600, abbreviation: "BRT" },
    TimezoneInfo { name: "America/Mexico_City", offset_seconds: -6 * 3600, abbreviation: "CST" },

    // Pacific
    TimezoneInfo { name: "Pacific/Auckland", offset_seconds: 13 * 3600, abbreviation: "NZDT" },
    TimezoneInfo { name: "Pacific/Sydney", offset_seconds: 11 * 3600, abbreviation: "AEDT" },
    TimezoneInfo { name: "Australia/Sydney", offset_seconds: 11 * 3600, abbreviation: "AEDT" },
    TimezoneInfo { name: "Australia/Melbourne", offset_seconds: 11 * 3600, abbreviation: "AEDT" },
    TimezoneInfo { name: "Pacific/Honolulu", offset_seconds: -10 * 3600, abbreviation: "HST" },
];

fn find_timezone(name: &str) -> Option<&'static TimezoneInfo> {
    TIMEZONES.iter().find(|tz| tz.name.eq_ignore_ascii_case(name))
}

fn format_offset(offset_seconds: i32) -> String {
    let sign = if offset_seconds >= 0 { '+' } else { '-' };
    let abs_offset = offset_seconds.abs();
    let hours = abs_offset / 3600;
    let minutes = (abs_offset % 3600) / 60;
    format!("{}{:02}:{:02}", sign, hours, minutes)
}

fn timestamp_to_datetime(timestamp_secs: i64, offset_seconds: i32) -> (i32, u32, u32, u32, u32, u32) {
    let adjusted_secs = timestamp_secs + offset_seconds as i64;

    // Days since Unix epoch
    let mut days = adjusted_secs / 86400;
    let mut remaining_secs = adjusted_secs % 86400;

    if remaining_secs < 0 {
        days -= 1;
        remaining_secs += 86400;
    }

    let hours = (remaining_secs / 3600) as u32;
    let minutes = ((remaining_secs % 3600) / 60) as u32;
    let seconds = (remaining_secs % 60) as u32;

    // Calculate date from days since epoch (1970-01-01)
    let mut year = 1970i32;
    let mut remaining_days = days;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let mut month = 1u32;
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month as i64 {
            break;
        }
        remaining_days -= days_in_month as i64;
        month += 1;
    }

    let day = remaining_days as u32 + 1;

    (year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn parse_time(time_str: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return Err("Time must be in HH:MM format".to_string());
    }

    let hours: u32 = parts[0].parse().map_err(|_| "Invalid hours")?;
    let minutes: u32 = parts[1].parse().map_err(|_| "Invalid minutes")?;

    if hours >= 24 {
        return Err("Hours must be 0-23".to_string());
    }
    if minutes >= 60 {
        return Err("Minutes must be 0-59".to_string());
    }

    Ok((hours, minutes))
}

// ============================================================================
// GetCurrentTime Tool
// ============================================================================

#[derive(Deserialize)]
struct GetCurrentTimeParams {
    timezone: String,
}

pub struct GetCurrentTimeTool;

impl GetCurrentTimeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GetCurrentTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GetCurrentTimeTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "get_current_time".to_string(),
            description: "Get current time in a specific timezone".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "description": "IANA timezone name (e.g., 'America/New_York', 'Europe/London'). Use 'UTC' as local timezone if no timezone provided by the user."
                    }
                },
                "required": ["timezone"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let params: GetCurrentTimeParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {}", e))?;

        let tz = find_timezone(&params.timezone)
            .ok_or_else(|| format!("Unknown timezone: {}. Use IANA timezone names like 'Asia/Ho_Chi_Minh', 'America/New_York', 'UTC'", params.timezone))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?;

        let timestamp_secs = now.as_secs() as i64;
        let (year, month, day, hours, minutes, seconds) = timestamp_to_datetime(timestamp_secs, tz.offset_seconds);

        let datetime = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
            year, month, day, hours, minutes, seconds,
            format_offset(tz.offset_seconds)
        );

        let result = serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "🕐 Current time in {} ({}):\n{}\n\nTimezone: {} ({})",
                    params.timezone,
                    tz.abbreviation,
                    datetime,
                    tz.name,
                    format_offset(tz.offset_seconds)
                )
            }]
        });

        Ok(result)
    }
}

// ============================================================================
// ConvertTime Tool
// ============================================================================

#[derive(Deserialize)]
struct ConvertTimeParams {
    time: String,
    source_timezone: String,
    target_timezone: String,
}

pub struct ConvertTimeTool;

impl ConvertTimeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConvertTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ConvertTimeTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "convert_time".to_string(),
            description: "Convert time between timezones".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "time": {
                        "type": "string",
                        "description": "Time to convert in 24-hour format (HH:MM)"
                    },
                    "source_timezone": {
                        "type": "string",
                        "description": "Source IANA timezone name (e.g., 'America/New_York', 'Europe/London'). Use 'UTC' as local timezone if no source timezone provided by the user."
                    },
                    "target_timezone": {
                        "type": "string",
                        "description": "Target IANA timezone name (e.g., 'Asia/Tokyo', 'America/San_Francisco'). Use 'UTC' as local timezone if no target timezone provided by the user."
                    }
                },
                "required": ["time", "source_timezone", "target_timezone"]
            }),
        }
    }

    fn execute(&self, params: serde_json::Value) -> McpResult<serde_json::Value> {
        let params: ConvertTimeParams =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {}", e))?;

        let source_tz = find_timezone(&params.source_timezone)
            .ok_or_else(|| format!("Unknown source timezone: {}", params.source_timezone))?;

        let target_tz = find_timezone(&params.target_timezone)
            .ok_or_else(|| format!("Unknown target timezone: {}", params.target_timezone))?;

        let (hours, minutes) = parse_time(&params.time)?;

        // Convert to UTC first (in minutes for simplicity)
        let source_minutes = (hours as i32 * 60 + minutes as i32) - (source_tz.offset_seconds / 60);

        // Then convert to target timezone
        let target_minutes = source_minutes + (target_tz.offset_seconds / 60);

        // Handle day overflow/underflow
        let mut adjusted_minutes = target_minutes;
        let mut day_diff = 0i32;

        if adjusted_minutes < 0 {
            day_diff = -1;
            adjusted_minutes += 24 * 60;
        } else if adjusted_minutes >= 24 * 60 {
            day_diff = 1;
            adjusted_minutes -= 24 * 60;
        }

        let target_hours = adjusted_minutes / 60;
        let target_mins = adjusted_minutes % 60;

        let day_note = match day_diff {
            -1 => " (previous day)",
            1 => " (next day)",
            _ => "",
        };

        let result = serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "🔄 Time Conversion:\n\n{} {} ({}) → {:02}:{:02} {} ({}){}\n\nSource: {} ({})\nTarget: {} ({})",
                    params.time,
                    source_tz.name,
                    source_tz.abbreviation,
                    target_hours,
                    target_mins,
                    target_tz.name,
                    target_tz.abbreviation,
                    day_note,
                    source_tz.name,
                    format_offset(source_tz.offset_seconds),
                    target_tz.name,
                    format_offset(target_tz.offset_seconds)
                )
            }]
        });

        Ok(result)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_current_time_utc() {
        let tool = GetCurrentTimeTool::new();
        let params = serde_json::json!({"timezone": "UTC"});
        let result = tool.execute(params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_current_time_vietnam() {
        let tool = GetCurrentTimeTool::new();
        let params = serde_json::json!({"timezone": "Asia/Ho_Chi_Minh"});
        let result = tool.execute(params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_current_time_invalid_timezone() {
        let tool = GetCurrentTimeTool::new();
        let params = serde_json::json!({"timezone": "Invalid/Timezone"});
        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_time_same_timezone() {
        let tool = ConvertTimeTool::new();
        let params = serde_json::json!({
            "time": "14:30",
            "source_timezone": "UTC",
            "target_timezone": "UTC"
        });
        let result = tool.execute(params);
        assert!(result.is_ok());
    }

    #[test]
    fn test_convert_time_vietnam_to_new_york() {
        let tool = ConvertTimeTool::new();
        let params = serde_json::json!({
            "time": "14:30",
            "source_timezone": "Asia/Ho_Chi_Minh",
            "target_timezone": "America/New_York"
        });
        let result = tool.execute(params);
        assert!(result.is_ok());
        // Vietnam is UTC+7, New York is UTC-5, so 12 hours difference
        // 14:30 Vietnam = 02:30 New York (same day)
    }

    #[test]
    fn test_convert_time_invalid_format() {
        let tool = ConvertTimeTool::new();
        let params = serde_json::json!({
            "time": "25:00",
            "source_timezone": "UTC",
            "target_timezone": "UTC"
        });
        let result = tool.execute(params);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_valid() {
        assert_eq!(parse_time("14:30"), Ok((14, 30)));
        assert_eq!(parse_time("00:00"), Ok((0, 0)));
        assert_eq!(parse_time("23:59"), Ok((23, 59)));
    }

    #[test]
    fn test_parse_time_invalid() {
        assert!(parse_time("24:00").is_err());
        assert!(parse_time("12:60").is_err());
        assert!(parse_time("invalid").is_err());
    }
}
