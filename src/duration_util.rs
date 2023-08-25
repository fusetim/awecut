use std::fmt::{Display, Formatter};

pub struct DurationDisplay(pub f32);

impl Display for DurationDisplay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let total_secs = self.0.round() as usize;
        let hours = total_secs / 3600;
        let rem = total_secs % 3600;
        let minutes = rem / 60;
        let seconds = rem % 60;
        let fraction = (self.0.fract() * 100.0) as usize;

        write!(f, "{}:{:02}:{:02}.{:02}", hours, minutes, seconds, fraction)
    }
}
