use chrono::{
    DateTime, FixedOffset, Local, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, Offset,
    TimeZone, Utc,
};
use chrono_tz::{TZ_VARIANTS, Tz};
use mlua::prelude::LuaString;
use mlua::{Error, Lua, Result, Table, UserData, UserDataFields, UserDataMethods, Value};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};

const MICROS: f64 = 1000000.0;

fn decode_mapped_time<T>(time: MappedLocalTime<T>) -> Result<T> {
    match time {
        MappedLocalTime::Single(time) => Ok(time),
        MappedLocalTime::Ambiguous(time, _) => Ok(time),
        MappedLocalTime::None => Err(Error::runtime("Failed to decode timestamp?")),
    }
}

#[derive(Copy, Clone, Debug)]
enum LuaTimezone {
    Local(Local),
    Tz(Tz),
}
impl LuaTimezone {
    pub fn from_str(name: &str) -> Result<LuaTimezone> {
        if name.eq_ignore_ascii_case("local") {
            Ok(LuaTimezone::Local(Local))
        } else {
            Ok(LuaTimezone::Tz(Tz::from_str_insensitive(name).map_err(Error::runtime)?))
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            LuaTimezone::Local(_) => "Local",
            LuaTimezone::Tz(tz) => tz.name(),
        }
    }
}
impl UserData for LuaTimezone {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Timezone");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, ()| Ok(this.name()));
    }
}
impl TimeZone for LuaTimezone {
    type Offset = LuaTzOffset;

    fn from_offset(offset: &Self::Offset) -> Self {
        match offset {
            LuaTzOffset::FixedOffset(off) => LuaTimezone::Local(Local::from_offset(off)),
            LuaTzOffset::TzOffset(off) => LuaTimezone::Tz(Tz::from_offset(off)),
        }
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<Self::Offset> {
        match self {
            LuaTimezone::Local(zone) => zone
                .offset_from_local_date(local)
                .map(LuaTzOffset::FixedOffset),
            LuaTimezone::Tz(zone) => zone
                .offset_from_local_date(local)
                .map(LuaTzOffset::TzOffset),
        }
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<Self::Offset> {
        match self {
            LuaTimezone::Local(zone) => zone
                .offset_from_local_datetime(local)
                .map(LuaTzOffset::FixedOffset),
            LuaTimezone::Tz(zone) => zone
                .offset_from_local_datetime(local)
                .map(LuaTzOffset::TzOffset),
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> Self::Offset {
        match self {
            LuaTimezone::Local(zone) => LuaTzOffset::FixedOffset(zone.offset_from_utc_date(utc)),
            LuaTimezone::Tz(zone) => LuaTzOffset::TzOffset(zone.offset_from_utc_date(utc)),
        }
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
        match self {
            LuaTimezone::Local(zone) => {
                LuaTzOffset::FixedOffset(zone.offset_from_utc_datetime(utc))
            }
            LuaTimezone::Tz(zone) => LuaTzOffset::TzOffset(zone.offset_from_utc_datetime(utc)),
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum LuaTzOffset {
    FixedOffset(FixedOffset),
    TzOffset(<Tz as TimeZone>::Offset),
}
impl Offset for LuaTzOffset {
    fn fix(&self) -> FixedOffset {
        match self {
            LuaTzOffset::FixedOffset(off) => off.fix(),
            LuaTzOffset::TzOffset(off) => off.fix(),
        }
    }
}
impl Display for LuaTzOffset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LuaTzOffset::FixedOffset(zone) => Display::fmt(zone, f),
            LuaTzOffset::TzOffset(zone) => Display::fmt(zone, f),
        }
    }
}

fn value_as_timezone(value: Value) -> Result<LuaTimezone> {
    match value {
        Value::Nil => Ok(LuaTimezone::Tz(Tz::UTC)),
        Value::String(name) => Ok(LuaTimezone::from_str(&name.to_str()?)?),
        Value::UserData(ud) if ud.is::<LuaTimezone>() => Ok(*ud.borrow::<LuaTimezone>()?),
        _ => Err(Error::runtime(format_args!(
            "found: {}, expected: nil, string or Timezone",
            value.type_name(),
        ))),
    }
}

fn create_tz_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    let new_mt = lua.create_table()?;
    new_mt.set("__metatable", false)?;
    new_mt.set(
        "__index",
        lua.create_function::<_, _, ()>(|_, (_, name): (Value, LuaString)| {
            Err(Error::runtime(format_args!("Timezone not found: {:?}", name.to_str()?)))
        })?,
    )?;
    new_mt.set_readonly(true);

    table.set_metatable(Some(new_mt.clone()))?;
    table.raw_set("Local", LuaTimezone::Local(Local))?;
    for variant in TZ_VARIANTS {
        table.raw_set(variant.name(), LuaTimezone::Tz(variant))?;
    }

    Ok(table)
}

const RFC_2822: &str = "%a, %d %b %Y %H:%M:%S %Z";
const RFC_3339: &str = "%Y-%m-%dT%H:%M:%S%#z";
const DEFAULT_FORMATS: &[&str] = &["%a, %d %b %Y %H:%M:%S%.f %Z", "%Y-%m-%dT%H:%M:%S%.f%#z"];

fn parse_format(value: &Option<LuaString>) -> Result<Cow<'static, str>> {
    match value {
        None => Ok(Cow::Borrowed(RFC_2822)),
        Some(str) => Ok(Cow::Owned(str.to_str()?.to_string())),
    }
}
fn parse_date(date: &str, format: &str, tz: LuaTimezone) -> Result<DateTime<LuaTimezone>> {
    if let Ok(date_time) = DateTime::parse_from_str(date, format) {
        Ok(date_time.with_timezone(&tz))
    } else if let Ok(naive) = NaiveDateTime::parse_from_str(date, format) {
        decode_mapped_time(naive.and_local_timezone(tz))
    } else if let Ok(naive) = NaiveDate::parse_from_str(date, format) {
        let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let naive = NaiveDateTime::new(naive, time);
        decode_mapped_time(naive.and_local_timezone(tz))
    } else {
        Err(Error::runtime("Could not parse date-time."))
    }
}
fn parse_input_format(date: &str, value: &Value, tz: LuaTimezone) -> Result<f64> {
    if value.is_nil() {
        for format in DEFAULT_FORMATS {
            if let Ok(value) = parse_date(date, *format, tz) {
                return Ok(value.timestamp_micros() as f64 / MICROS);
            }
        }
        Err(Error::runtime(format!("No formats matched date: {date}")))
    } else if let Some(value) = value.as_string() {
        let value = parse_date(date, &value.to_str()?, tz)?;
        return Ok(value.timestamp_micros() as f64 / MICROS);
    } else if let Some(value) = value.as_table() {
        for v in value.clone().sequence_values::<Value>() {
            let v = v?;
            if let Ok(value) = parse_date(date, v.to_string()?.as_str(), tz) {
                return Ok(value.timestamp_micros() as f64 / MICROS);
            }
        }
        Err(Error::runtime(format!("No formats matched date: {date}")))
    } else {
        Err(Error::runtime("Input format must be nil, {string}, or string"))
    }
}

pub fn create_date_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    table.raw_set("Timezone", create_tz_table(lua)?)?;

    table.raw_set("rfc2822", RFC_2822)?;
    table.raw_set("rfc3339", RFC_3339)?;
    table.raw_set("iso8601", RFC_3339)?;

    table.raw_set(
        "now_timestamp",
        lua.create_function(|_, ()| {
            let time = Utc::now();
            Ok(time.timestamp())
        })?,
    )?;
    table.raw_set(
        "now_timestamp_frac",
        lua.create_function(|_, ()| {
            let time = Utc::now();
            Ok(time.timestamp_micros() as f64 / MICROS)
        })?,
    )?;
    table.raw_set(
        "format",
        lua.create_function(|_, (time, format, tz): (f64, Option<LuaString>, Value)| {
            let tz = value_as_timezone(tz)?;
            let time = decode_mapped_time(tz.timestamp_micros((time * MICROS) as i64))?;
            Ok(time.format(&parse_format(&format)?).to_string())
        })?,
    )?;
    table.raw_set(
        "to_timestamp",
        lua.create_function(|_, (str, format, tz): (LuaString, Value, Value)| {
            let tz = value_as_timezone(tz)?;
            parse_input_format(&str.to_str()?, &format, tz)
        })?,
    )?;

    Ok(table)
}
