use clap::Parser;
use json::JsonValue;
use rand_seeder::Seeder;
use rand_xoshiro::Xoshiro256PlusPlus;
use std::{cmp::Ordering, collections::{BinaryHeap, VecDeque}, fs, io::{self, Write}, process::exit};

mod args;
mod cleaning_crew;
mod room_data;
use crate::args::Args;
use crate::cleaning_crew::CleaningCrew;
use crate::room_data::RoomData;

// *nix terminal text styling sequences
static RED: &str = "\x1B[31m";
static CLEAR: &str = "\x1B[0m";

fn main() {
    let args = Args::parse();

    let json_path = args.json_path.clone();
    let maybe_text = fs::read_to_string(args.json_path);

    let json: json::JsonValue;
    match maybe_text {
        Ok(text) => {
            match json::parse(&text) {
                Ok(parsed_json) => json = parsed_json,
                Err(e) => {
                    _ = writeln!(io::stderr(), "Unable to parse json: {e:?}");
                    exit(2);
                }
            }
        },
        Err(e) => {
            let json_path = json_path.to_str().unwrap();
            _ = writeln!(io::stderr(), "Unable to read file {json_path}: {e:?}");
            exit(1);
        }
    }

    let rooms = handle_json_data(json, &args.seed);
    process_rooms(rooms, args.cleaning_crews, args.hire_crews);
}

fn process_rooms(rooms: Vec<RoomData>, cleaning_crews: i32, hire_crews: bool) {
    if rooms.len() == 0 {
        println!("All rooms are clean.");
        return;
    }

    if cleaning_crews < 1 && !hire_crews {
        println!("Nobody to clean our rooms :(");
        return;
    }

    // 1. split into reserved and unreserved rooms.
    // 2. order reserved rooms by both customer arrival time and earliest possible cleaning end
    // 3. choose a room to clean
    //   3.1 if the first room in any of the lists is already cleaned for some reason, discard it and repeat
    //   3.2 if the first reserved room by customer arrival is unoccupied (current time >= occupied_until), proceed with it to step 3.6
    //   3.3 if the first reserved room by cleaning end is unoccupied, proceed with it to step 3.6
    //   3.4 if the first unreserved room is unoccupied, proceed with it to step 3.6
    //   3.5 No unoccupied rooms exist currently among the first rooms. Take the room among them that becomes unoccupied next and proceed with it to step 3.6
    //   3.6 clean the room (advance time by cleaning time, set room to cleaning_completed = true)
    //   3.7 print the room information, current time and whether or not the guests arrived before cleaning completed
    // 4. if there are rooms left, return to step 3

    let mut unreserved_rooms = Vec::new();
    let mut reserved_rooms = Vec::new();
    let mut indefinitely_occupied_rooms = 0;
    split_rooms_by_occupancy(rooms, &mut unreserved_rooms, &mut reserved_rooms, &mut indefinitely_occupied_rooms);

    // shadow variable to make it non-mut
    let indefinitely_occupied_rooms = indefinitely_occupied_rooms;

    let mut reserved_rooms_by_cleaning_end = sorted_indices(
        reserved_rooms.len(),
        |a, b| reserved_rooms[*a].cleaning_end().cmp(&reserved_rooms[*b].cleaning_end()));
    let mut reserved_rooms_by_customer_arrival = sorted_indices(
        reserved_rooms.len(),
        |a, b| reserved_rooms[*a].latest_cleaning_start().cmp(&reserved_rooms[*b].latest_cleaning_start()));

    let mut unreserved_index = 0;
    let mut current_times = BinaryHeap::<CleaningCrew>::new();
    for crew in 0..cleaning_crews {
        current_times.push(CleaningCrew::new(crew));
    }

    let mut hired_crews = 0;
    if hire_crews && cleaning_crews == 0 {
        hired_crews += 1;
        current_times.push(CleaningCrew::new(0));
    }

    let mut late_rooms = 0;
    loop {
        let mut crew = current_times.peek().unwrap();
        let mut current_time = crew.occupied_until();
        
        let source = get_room_to_clean_source(
            &reserved_rooms_by_customer_arrival, 
            &reserved_rooms_by_cleaning_end,
            &reserved_rooms,
            &unreserved_rooms,
            unreserved_index,
            current_time);

        let room = match source {
            RoomToCleanSource::None => None,
            RoomToCleanSource::ByArrival => reserved_rooms.get_mut(reserved_rooms_by_customer_arrival.pop_front().unwrap()),
            RoomToCleanSource::ByCleaningEnd => reserved_rooms.get_mut(reserved_rooms_by_cleaning_end.pop_front().unwrap()),
            RoomToCleanSource::Unreserved => { let room = unreserved_rooms.get_mut(unreserved_index); unreserved_index += 1; room },
        };

        if room.is_none() {
            break;
        }

        let room = room.unwrap();

        if room.cleaning_completed {
            continue;
        }

        let new_crew = if hire_crews && current_time > room.latest_cleaning_start() {
            hired_crews += 1;
            let new_crew = CleaningCrew::new(cleaning_crews + hired_crews);
            current_time = room.occupied_until().unwrap_or(0);

            print!("hiring! ");
            Some(new_crew)
        }
        else {
            None
        };

        if let Some(ref new_crew) = new_crew {
            crew = new_crew;
        }

        current_time = room.clean(current_time);
        let msg;
        if current_time <= room.reserved_from().unwrap_or(i32::MAX) {
            msg = String::from("");
        }
        else {
            msg = format!(" {RED}LATE!!{CLEAR}");
            late_rooms += 1;
        }

        let crew_id = crew.id();
        let crew = crew.clean_until(current_time);
        if new_crew.is_none() {
            current_times.pop();
        }
        
        current_times.push(crew);

        println!("{crew_id} @ {current_time}: Cleaned {room}{msg}");
    }

    if late_rooms > 0 {
        println!("{RED}{late_rooms}{CLEAR} rooms were late! 💩");
    }

    if hired_crews > 0 {
        println!("Had to hire {RED}{hired_crews}{CLEAR} new crews.");
    }

    if indefinitely_occupied_rooms > 0 {
        println!("{indefinitely_occupied_rooms} rooms need cleaning but are occupied indefinitely.");
    }
}

#[derive(Copy, Clone)]
enum RoomToCleanSource {
    Unreserved,
    ByArrival,
    ByCleaningEnd,
    None
}

fn get_room_to_clean_source(
    by_arrival: &VecDeque<usize>,
    by_cleaning_end: &VecDeque<usize>,
    reserved_rooms: &Vec<RoomData>,
    unreserved_rooms: &Vec<RoomData>,
    unreserved_rooms_index: usize,
    current_time: i32)
-> RoomToCleanSource {
    let room_by_arrival = get_occupied_room(by_arrival, reserved_rooms);
    let room_by_cleaning_end = get_occupied_room(by_cleaning_end, reserved_rooms);
    let unreserved_room = unreserved_rooms.get(unreserved_rooms_index);

    // check whether we need to move some already cleaned rooms out of the way first
    if room_by_arrival.is_some_and(|r| r.cleaning_completed) {
        return RoomToCleanSource::ByArrival;
    }

    if room_by_cleaning_end.is_some_and(|r| r.cleaning_completed) {
        return RoomToCleanSource::ByCleaningEnd;
    }

    if unreserved_room.is_some_and(|r| r.cleaning_completed) {
        return RoomToCleanSource::Unreserved;
    }

    // check whether any room is a best possible candidate by being available for cleaning immediately
    if room_by_arrival.is_some_and(|r| r.occupied_until().unwrap_or(0) <= current_time) {
        return RoomToCleanSource::ByArrival;
    }

    if room_by_cleaning_end.is_some_and(|r| r.occupied_until().unwrap_or(0) <= current_time) {
        return RoomToCleanSource::ByCleaningEnd;
    }

    if unreserved_room.is_some_and(|r| r.occupied_until().unwrap_or(0) <= current_time) {
        return RoomToCleanSource::Unreserved;
    }

    // fallback: wait for the next available room
    let mut rooms = Vec::<(&RoomData, RoomToCleanSource)>::new();
    if let Some(room) = room_by_arrival {
        rooms.push((room, RoomToCleanSource::ByArrival));
    }

    if let Some(room) = room_by_cleaning_end {
        rooms.push((room, RoomToCleanSource::ByCleaningEnd));
    }

    if let Some(room) = unreserved_room {
        rooms.push((room, RoomToCleanSource::Unreserved));
    }

    rooms.sort_by(|l, r| l.0.cleaning_end().cmp(&r.0.cleaning_end()));
    if let Some(room) = rooms.get(0) {
        room.1
    }
    else {
        RoomToCleanSource::None
    }
}

fn get_occupied_room<'a>(indices: &VecDeque<usize>, rooms: &'a Vec<RoomData>) -> Option<&'a RoomData> {
    let index = indices.front()?;
    rooms.get(*index)
}

fn sorted_indices<F>(len: usize, sorter: F) -> VecDeque<usize>
    where F: FnMut(&usize, &usize) -> Ordering {
    let mut sortable = Vec::new();
    for idx in 0..len {
        sortable.push(idx);
    }

    sortable.sort_by(sorter);

    sortable.into_iter().collect::<VecDeque<usize>>()
}

fn split_rooms_by_occupancy(rooms: Vec<RoomData>, unreserved_rooms: &mut Vec<RoomData>, reserved_rooms: &mut Vec<RoomData>, indefinitely_occupied_rooms: &mut u32) {
    for room in rooms {
        if let Some(-1) = room.occupied_until() {
            *indefinitely_occupied_rooms += 1;
        }
        else if let Some(_) = room.reserved_from() {
            reserved_rooms.push(room);
        }
        else {
            unreserved_rooms.push(room);
        }
    }
}

fn handle_json_data(json: JsonValue, seed: &Option<String>) -> Vec<RoomData> {
    if let JsonValue::Array(array) = json {
        let mut rooms = Vec::<RoomData>::new();
        let mut rng: Option<Xoshiro256PlusPlus> = match seed {
            None => None,
            Some(s) if s.len() == 0 => None,
            Some(s) => Some(Seeder::from(s).make_rng()),
        };
        
        for item in array {
            if let Some(room) = RoomData::from_json(&item, &mut rng) {
                // filter out rooms that don't need cleaning at all
                match room.cleaning_duration() {
                    Some(0) => {},
                    Some(_) => { rooms.push(room); }
                    _ => {}
                }
            }
            else {
                _ = writeln!(io::stderr(), "Invalid object data: {item}!");
                exit(4);
            }
        }
        
        rooms
    }
    else {
        let actual_type = match json {
            JsonValue::Short(v) => format!("{v} (short)"),
            JsonValue::Boolean(b) => format!("{b} (boolean)"),
            JsonValue::Array(_) => panic!("got array after already checking for array!"),
            JsonValue::Null => String::from("null"),
            JsonValue::Number(n) => format!("{n} (number)"),
            JsonValue::Object(_) => {
                format!("{json}")
            },
            JsonValue::String(s) => format!("{s} (string)"),
        };

        _ = writeln!(io::stderr(), "Expected array, got {actual_type}!");
        exit(3);
    }
}