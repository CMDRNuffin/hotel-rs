use std::fmt::{self, Display};
use json::JsonValue;
use rand::prelude::*;

pub struct RoomData {
    room_number: i32,
    occupied_until: Option<i32>,
    reserved_from: Option<i32>,
    cleaning_duration: Option<i32>,
    pub cleaning_completed: bool,
}

impl RoomData {
    pub fn cleaning_end(&self) -> i32 {
        self.occupied_until.unwrap_or(0) + self.cleaning_duration.unwrap_or(0)
    }

    pub fn latest_cleaning_start(&self) -> i32 {
        match (self.cleaning_duration, self.reserved_from) {
            (None, _) => i32::MAX,
            (_, None) => i32::MAX,
            (Some(cleaning_duration), Some(reserved_from)) => reserved_from - cleaning_duration
        }
    }

    pub fn from_json<T>(obj: &JsonValue, rng: &mut Option<T>) -> Option<Self> where T : Rng {
        let room_number = obj["roomNumber"].as_i32()?;
        let is_occupied = obj["isOccupied"].as_bool()?;
        let occupied_until = obj["occupiedUntil"].as_i32()?;
        let is_reserved = obj["isReserved"].as_bool()?;
        let needs_cleaning = obj["hasToBeCleaned"].as_bool()?;
        let cleaning_duration = obj["cleaningDuration"].as_i32()?;

        let reservation_start;
        if !is_reserved {
            reservation_start = 0;
        }
        else {
            let occupation_end = if is_occupied { occupied_until } else { 0 };
            let cleaning_duration = if needs_cleaning { cleaning_duration } else { 0 };
            let earliest_reservation_start = occupation_end + cleaning_duration;
            let rnd = match rng {
                None => 500 + rand::random::<i32>() % 19500,
                Some(r) => r.gen_range(500..20000)
            };
            reservation_start = earliest_reservation_start + rnd;
        }

        Some(RoomData {
            room_number: room_number,
            occupied_until: if is_occupied { Some(occupied_until) } else { None },
            reserved_from: if is_reserved && (!is_occupied || occupied_until >= 0) { Some(reservation_start) } else { None },
            cleaning_duration: if needs_cleaning { Some(cleaning_duration) } else { None },
            cleaning_completed: false,
        })
    }
    
    pub fn occupied_until(&self) -> Option<i32> {
        self.occupied_until
    }

    pub fn cleaning_duration(&self) -> Option<i32> {
        self.cleaning_duration
    }

    pub fn reserved_from(&self) -> Option<i32> {
        self.reserved_from
    }

    pub fn clean(&mut self, earliest_start: i32) -> i32 {
        self.cleaning_completed = true;

        if self.cleaning_duration.is_none()
        {
            return earliest_start;
        }

        let duration = self.cleaning_duration.unwrap_or(0);
        if duration == 0 {
            return earliest_start;
        }

        let start = match self.occupied_until {
            None => earliest_start,
            Some(to) if to < earliest_start => earliest_start,
            Some(to) => to
        };

        start + duration
    }
}

impl Display for RoomData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let room_number = self.room_number;
        let occupied_until = match self.occupied_until {
            None => String::from("unoccupied"),
            Some(-1) => String::from("occupied indefinitely"),
            Some(end) => format!("occupied until {end}"),
        };
        let reserved_from = match self.reserved_from {
            None =>  String::from("unreserved"),
            Some(0) => String::from("reserved"),
            Some(from) => format!("reserved from {from}"),
        };
        let cleaning_duration = match self.cleaning_duration {
            None => String::from("clean"),
            Some(duration) => format!("requires cleaning for {duration} ticks"),
        };
        write!(f, "#{room_number} {occupied_until} {reserved_from} {cleaning_duration}")?;
        Ok(())
    }
}