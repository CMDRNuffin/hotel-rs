use std::cmp::Ordering;

#[derive(PartialEq, Eq)]
pub struct CleaningCrew {
    id: i32,
    occupied_until: i32
}

impl CleaningCrew {
    pub fn new(id: i32) -> Self {
        CleaningCrew {
            id: id,
            occupied_until: 0,
        }
    }

    pub fn clean_until(&self, cleaning_end: i32) -> Self {
        CleaningCrew {
            id: self.id,
            occupied_until: cleaning_end,
        }
    }

    pub fn occupied_until(&self) -> i32 {
        self.occupied_until
    }

    pub fn id(&self) -> i32 {
        self.id
    }
}

impl Ord for CleaningCrew {
    fn cmp(&self, other: &Self) -> Ordering {
        // order by occupied_until desc, id desc (by reversing the operands)
        // since BinaryHeap is a "largest first" priority queue and I didn't want to deal with wrapping all the items in Reverse()
        // which reverses ordering but requires unwrapping to access the contents again
        other.occupied_until.cmp(&self.occupied_until)
            .then(other.id.cmp(&self.id))
    }
}

// We also need to implement PartialOrd, which can just delegate to Ord since a definite ordering exists and is fast to compute
impl PartialOrd for CleaningCrew {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}