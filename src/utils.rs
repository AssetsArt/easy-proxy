use openssl::{
    asn1::{Asn1Time, Asn1TimeRef},
    error::ErrorStack,
};
use std::time::{Duration, SystemTime};

pub fn asn1_time_to_unix_time(time: &Asn1TimeRef) -> Result<i128, ErrorStack> {
    let threshold = Asn1Time::days_from_now(0).unwrap();
    let time = threshold.diff(time)?;
    let days = time.days; // Difference in days
    let seconds = time.secs; // This is always less than the number of seconds in a day.
    let duration = Duration::from_secs((days as u64) * 86400 + seconds as u64);
    let epoch = SystemTime::UNIX_EPOCH;
    let since_the_epoch = match epoch.checked_add(duration) {
        Some(val) => val,
        None => {
            return Err(ErrorStack::get());
        }
    };
    let time = chrono::DateTime::<chrono::Utc>::from(since_the_epoch).timestamp() as i128;
    let now = chrono::Utc::now().timestamp() as i128;
    Ok(time + now)
}
