use std::collections::BTreeMap;
use rand::Rng;

/// The characteristics of the minefield
#[derive(Clone, Debug)]
pub struct Minefield {
    /// The mine field as a set of coords `(x, y)` associated with a `Spot`
    field: BTreeMap<(u16, u16), Spot>,

    /// Number of mines in the field
    mines: u32,

    /// Width of field grid
    width: u16,

    /// Height of field grid
    height: u16,
}

impl Minefield {
    /// Create an empty minefield grid (with all spots hidden), with the given width and height
    pub fn new(width: u16, height: u16) -> Self {
        // Enforce a minimum number of spots
        let width = if width == 0 { 1 } else { width };
        let height = if height == 0 { 1 } else { height };
    
        let field =  
            (0..=width)
            .flat_map(move |i| {
                (0..=height).map(move |j| (i, j))
            })
            .map(|(x, y)| {
                ((x, y), Spot::default())
            })
            .collect();
            
        // Create empty Minefield
        Minefield {
            field,
            mines: 0,
            width,
            height,
        }
    }

    /// Build an existing minefield with the given number of mines randomly placed in it
    pub fn with_mines(mut self, mines: u32) -> Self {
        // Total number of spots in our field
        let spot_count = self.width as usize * self.height as usize;

        // Limit the max number of mines to the number of available spots
        let mines = if mines as usize <= spot_count { mines } else { spot_count as u32 };

        self.mines = mines;

        // Add mines to minefield

        // We could just start randomly picking indices in the field and hope we haven't picked them before, but if a
        // user desires a field full of mines, then waiting for the last mines to be placed might take a long time
        // (e.g. if the field is very large).
        // That's a problem for an immediate GUI.
        // So, instead, we'll use some memory in order to ensure that the user can step on a mine as soon as humanly
        // possible.
        let mut spots_remaining: Vec<usize> = (0..spot_count).collect();
        let mut rng = rand::thread_rng();

        // Place mines
        for _ in 0..self.mines {
            let index_rm = rng.gen_range(0..spots_remaining.len());
            let index = spots_remaining.swap_remove(index_rm);
            let x = (index as u32 % self.width as u32) as u16;
            let y = (index as u32 / self.width as u32) as u16;
            self.place_mine(x, y);
        }

        self
    }

    /// Step on a given spot of the field. Coordinates [x=0, y=0] represent the top-left point of the field grid
    pub fn step(&mut self, x: u16, y: u16) -> StepResult {
        let step_result;

        if let Some(spot) = self.field.get_mut(&(x, y)) {
            step_result = spot.step();
        } else {
            // Step is outside minefield
            step_result = StepResult::Invalid
        }

        // flood reveal if this is an empty spot with no neighboring mines
        if let StepResult::Phew = step_result {
            if let SpotState::RevealedEmpty { neighboring_mines } = self.field.get(&(x, y)).unwrap().state {
                if neighboring_mines == 0 {
                    let mut spots_to_visit = vec![(x, y)];

                    while let Some((xx, yy)) = spots_to_visit.pop() {                            
                        for (nb_x, nb_y) in self.neighbors_coords(xx, yy) {
                            let spot = self.field.get_mut(&(nb_x, nb_y)).unwrap();
                            
                            if let SpotState::HiddenEmpty { neighboring_mines } = spot.state {
                                spot.state = SpotState::RevealedEmpty { neighboring_mines };

                                if neighboring_mines == 0 {
                                    spots_to_visit.push((nb_x, nb_y));
                                }
                            }
                        }
                    }
                }
            }
        }
            
        step_result
    }

    /// Automatically step on all hidden neighbors (i.e. not flagged) of a revealed spot at the given coordiantes
    pub fn auto_step(&mut self, x: u16, y: u16) -> StepResult {
        if let Some(spot) = self.field.get(&(x, y)) {
            if let SpotState::RevealedEmpty { neighboring_mines } = spot.state {
                 // count the flags around the given coords
                 let placed_flags = self
                    .neighbors_coords(x, y)
                    .filter(|(x, y)| {
                        match self.field.get(&(*x, *y)).unwrap().state {
                            SpotState::FlaggedEmpty { neighboring_mines: _ } => true,
                            SpotState::FlaggedMine => true,
                            _ => false,
                        }
                    })
                    .count() as u8;
                            
                // only try to autostep if the user has placed enough flags around the step whose neighbors will be autorevealed
                if placed_flags == neighboring_mines {
                    for (nx, ny) in self.neighbors_coords(x, y) {
                        let step_result = self.field.get_mut(&(nx, ny)).unwrap().step();

                        if StepResult::Boom == step_result {
                            return step_result;
                        }
                    }
                }

                StepResult::Phew
            } else {
                StepResult::Invalid
            }
        } else {
            StepResult::Invalid
        }
    }

    /// Check if the minefield has been cleared
    pub fn is_cleared(&self) -> bool {
        for (_, spot) in &self.field {
            if !spot.is_resolved() {
                return false;
            }
        }
        
        true
    }

    /// Set a flag on a hidden spot, or clear the flag if the spot had one, or do nothing if
    /// the spot cannot be flagged
    pub fn toggle_flag(&mut self, x: u16, y: u16) -> FlagToggleResult {
        if let Some(spot) = self.field.get_mut(&(x, y)) {
            spot.flag()
        } else {
            // invalid coordinates, no flag was added or removed
            FlagToggleResult::None
        }
    }

    /// The width of the minefield
    pub fn width(&self) -> u16 {
        self.width as u16
    }

    /// The height of the minefield
    pub fn height(&self) -> u16 {
        self.height as u16
    }

    /// The number of mines in the minefield
    pub fn mines(&self) -> u32 {
        self.mines
    }    

    /// Place a mine at a given field coordiantes, and update neighboring spots
    fn place_mine(&mut self, x: u16, y: u16) {
        
        assert!(x < self.width);
        assert!(y < self.height);
        
        if let Some(spot) = self.field.get_mut(&(x, y)) {
            match spot.state {
                // Only place a mine in an emty field
                SpotState::HiddenEmpty { neighboring_mines: _ } | SpotState::FlaggedEmpty { neighboring_mines: _ } | SpotState::RevealedEmpty { neighboring_mines: _ } => {
                    spot.state = SpotState::HiddenMine;
                    
                    // Update counts of empty neighboring spots
                    for (nx, ny) in self.neighbors_coords(x, y) {
                        if let Some(spot) = self.field.get_mut(&(nx, ny)) {
                            match &mut spot.state {
                                // Only place a mine in an emty field
                                SpotState::HiddenEmpty { neighboring_mines } | SpotState::FlaggedEmpty { neighboring_mines } | SpotState::RevealedEmpty { neighboring_mines } => {
                                    *neighboring_mines += 1;
                                },
                                _ => {},
                            }
                        }
                    }                    
                },
                _ => {},
            }
        }
    }

    /// Iterator over the coordinates of all neighbors in a range of 1 unit, relative to the given coordiantes
    fn neighbors_coords(&self, x: u16, y: u16) -> impl Iterator<Item = (u16, u16)>
    {        
        let min_x = x.saturating_sub(1);
        let max_x = x.saturating_add(1);

        let min_y = y.saturating_sub(1);
        let max_y = y.saturating_add(1);

        let width = self.width;
        let height = self.height;

        (min_x..=max_x)
            .flat_map(move |i| {
                (min_y..=max_y).map(move |j| (i, j))
            })
            .filter(move |(neighbor_x, neighbor_y)| {
                *neighbor_x < width && 
                *neighbor_y < height && 
                !(*neighbor_x == x && *neighbor_y == y)
            })       
    }
}

/// State of the spot in a minefield
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SpotState {
    /// This spot has not been visited
    HiddenEmpty{neighboring_mines: u8},

    /// This is a hidden mine
    HiddenMine,

    /// This spot is empty, but was flagged as a mine
    FlaggedEmpty{neighboring_mines: u8},

    /// This spot contains a mine which was correctly flagged
    FlaggedMine,

    /// This spot is empty and was correctly revealed
    RevealedEmpty{neighboring_mines: u8},

    /// This spot contains a mine and was incorrectly revealed
    ExplodedMine,
}

/// Spot struct describing the characteristics of the minefield at a particular position
#[derive(Copy, Clone, Debug)]
pub struct Spot {
    pub state: SpotState,
}

impl Spot {
    /// Step on this spot, if possible
    pub fn step(&mut self) -> StepResult {
        match self.state {
            SpotState::HiddenEmpty { neighboring_mines } => {
                self.state = SpotState::RevealedEmpty { neighboring_mines };
                StepResult::Phew
            },
            SpotState::HiddenMine => {
                self.state = SpotState::ExplodedMine;
                StepResult::Boom
            },
            _ => {
                StepResult::Invalid
            }
        }
    }

    /// Toggle a flag this spot, if possible
    pub fn flag(&mut self) -> FlagToggleResult {
        match self.state {
            SpotState::HiddenEmpty { neighboring_mines } => {
                self.state = SpotState::FlaggedEmpty { neighboring_mines };
                FlagToggleResult::Added
            },
            SpotState::HiddenMine => {
                self.state = SpotState::FlaggedMine {};
                FlagToggleResult::Added
            },
            SpotState::FlaggedEmpty { neighboring_mines } => {
                self.state = SpotState::HiddenEmpty { neighboring_mines };
                FlagToggleResult::Removed
            },
            SpotState::FlaggedMine => {
                self.state = SpotState::HiddenMine {};
                FlagToggleResult::Removed
            },
            _ => {
                FlagToggleResult::None
            }
        }
    }

    /// Has this spot been cleared (either correctly flagged or correctly revealed)?
    pub fn is_resolved(&self) -> bool {
        match self.state {
            SpotState::FlaggedMine => true,
            SpotState::RevealedEmpty { neighboring_mines: _ } => true,
            _ => false,
        }
    }
}

impl Default for Spot {
    fn default() -> Self {
        Self { state: SpotState::HiddenEmpty { neighboring_mines: 0 } }
    }
}

/// The result of steppin on a spot in the minefield
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StepResult {
    /// Stepped on empty spot
    Phew,

    /// Stepped on a mine
    Boom,

    /// Step not taken
    Invalid
}

/// The result of toggling a flag in the mine field
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FlagToggleResult {
    /// Exstng flag was removed
    Removed,
    /// A flag was added
    Added,
    /// No flag placed or removed
    None
}

 #[cfg(test)]
 mod tests {
    use super::*;

     #[test]
     fn new_minefield() {
        // Create empty test minefield:
        //     0 1 2
        // 0 [       ]
        // 1 [       ]
        // 2 [       ]
        // 3 [       ]
        //
        let width = 3;
        let height = 4;
        let minefield = Minefield::new(width, height);

        for (_, spot) in &minefield.field {
            assert_eq!(spot.state, SpotState::HiddenEmpty { neighboring_mines: 0 });
        }
     }

     #[test]
     fn place_mines() {
         // Create empty minefield
        let width = 3;
        let height = 4;
        let mut minefield = Minefield::new(width, height);

        // Place Mine
        //     0 1 2
        // 0 [   1 ☢ ]
        // 1 [   1 1 ]
        // 2 [       ]
        // 3 [       ]
        //
        let mine_x = 2;
        let mine_y = 0;
        minefield.place_mine(mine_x, mine_y);

        // Was mine placed correctly?
        assert_eq!(minefield.field.get(&(mine_x, mine_y)).unwrap().state, SpotState::HiddenMine);

        // Were the neighbors updated correctly?
        for (nx, ny) in minefield.neighbors_coords(mine_x, mine_y) {
            assert_eq!(minefield.field.get(&(nx, ny)).unwrap().state, SpotState::HiddenEmpty { neighboring_mines: 1 });
        }

        // Place another mine
        //     0 1 2
        // 0 [   1 ☢ ]
        // 1 [   1 1 ]
        // 2 [ 1 1   ]
        // 3 [ ☢ 1   ]
        let mine_x = 0;
        let mine_y = 3;
        minefield.place_mine(mine_x, mine_y);

        // Was mine placed correctly?
        assert_eq!(minefield.field.get(&(mine_x, mine_y)).unwrap().state, SpotState::HiddenMine);

        // Were the neighbors updated correctly?
        for (nx, ny) in minefield.neighbors_coords(mine_x, mine_y) {
            assert_eq!(minefield.field.get(&(nx, ny)).unwrap().state, SpotState::HiddenEmpty { neighboring_mines: 1 });
        }

        // Place another mine
        //     0 1 2
        // 0 [ 1 2 ☢ ]
        // 1 [ ☢ 2 1 ]
        // 2 [ 2 2   ]
        // 3 [ ☢ 1   ]
        let mine_x = 0;
        let mine_y = 1;
        minefield.place_mine(mine_x, mine_y);

        // Was mine placed correctly?
        assert_eq!(minefield.field.get(&(mine_x, mine_y)).unwrap().state, SpotState::HiddenMine);

        // Were the neighbors updated correctly?
        for n_coords in minefield.neighbors_coords(mine_x,  mine_y) {
            let expected_mine_count = if n_coords == (0, 0) { 1 } else { 2 };
            assert_eq!(minefield.field.get(&n_coords).unwrap().state, SpotState::HiddenEmpty { neighboring_mines: expected_mine_count });
        }
     }

     #[test]
     fn step() {
         // Create empty minefield
         let width = 3;
         let height = 4;
         let mut minefield = Minefield::new(width, height);

        // Place mines
        //     0 1 2
        // 0 [   1 ☢ ]
        // 1 [   1 1 ]
        // 2 [ 1 1   ]
        // 3 [ ☢ 1   ]
        let mine_x = 2;
        let mine_y = 0;
        minefield.place_mine(mine_x, mine_y);
        let mine_x = 0;
        let mine_y = 3;
        minefield.place_mine(mine_x, mine_y);

        // Step on spot neighboring mine
        let step_x = 1;
        let step_y = 2;
        let step_result = minefield.step(step_x, step_y);

        // Step was success, and only one spot was revealed
        //     0 1 2
        // 0 [ • • • ]
        // 1 [ • • • ]
        // 2 [ • 1 • ]
        // 3 [ • • • ]
        assert_eq!(step_result, StepResult::Phew);
        // assert_eq!(minefield.field[step_x as usize][step_y as usize].state, SpotState::Revealed);
        // for (nx, ny) in minefield.neighbors_coords(step_x, step_y) {
        //     assert_eq!(minefield.field[nx as usize][ny as usize].state, SpotState::Hidden);
        // }

        // Step on spot with no neighboring mines
        let step_x = 0;
        let step_y = 1;
        let step_result = minefield.step(step_x, step_y);

        // Step was success, and neighbors were flood revealed
        //     0 1 2
        // 0 [   1 • ]
        // 1 [   1 • ]
        // 2 [ 1 1 • ]
        // 3 [ • • • ]
        assert_eq!(step_result, StepResult::Phew);
        // assert_eq!(minefield.field[step_x as usize][step_y as usize].state, SpotState::Revealed);
        // for (nx, ny) in minefield.neighbors_coords(step_x, step_y) {
        //     assert_eq!(minefield.field[nx as usize][ny as usize].state, SpotState::Revealed);
        // }

        // Step on mine
        let step_x = 2;
        let step_y = 0;
        let step_result = minefield.step(step_x, step_y);

        // Step was Boom, and only mine spot was newly revealed
        //     0 1 2
        // 0 [   1 ☢ ]
        // 1 [   1 • ]
        // 2 [ 1 1 • ]
        // 3 [ • • • ]
        assert_eq!(step_result, StepResult::Boom);
        // assert_eq!(minefield.field[step_x as usize][step_y as usize].state, SpotState::Exploded);
        // for (x, y) in minefield.neighbors_coords(step_x,  step_y) {
        //     let expected_spot_state= if (x, y) == (2, 1) { SpotState::Hidden } else { SpotState::Revealed };
        //     assert_eq!(minefield.field[x as usize][y as usize].state, expected_spot_state);
        // }
     }

     #[test]
     fn flood_reveal() {
        // Create empty bigger minefield
        //     0 1 2 3 4 5 6 7 8 9
        // 0 [     1 ☢ 1           ]
        // 1 [     1 1 1           ]
        // 2 [           1 1 1     ]
        // 3 [   1 1 1   1 ☢ 1 1 1 ]
        // 4 [   1 ☢ 1   1 1 1 1 ☢ ]
        // 5 [   1 1 1         1 1 ]
        // 6 [         1 1 2 1 1   ]
        // 7 [         1 ☢ 2 ☢ 1   ]
        // 8 [         1 1 2 1 1   ]
        // 9 [                     ]
        let width = 10;
        let height = 10;
        let mut minefield = Minefield::new(width, height);

        let mine_coords = [(2, 4), (5, 7), (7, 7), (9, 4), (6, 3), (3, 0)];
        for (x, y) in mine_coords {
            minefield.place_mine(x, y);
        }

        // Place a flag
        //     0 1 2 3 4 5 6 7 8 9
        // 0 [ • • • • • • • • • • ]
        // 1 [ • • • • • ⚐ • • • • ]
        // 2 [ • • • • • • • • • • ]
        // 3 [ • • • • • • • • • • ]
        // 4 [ • • • • • • • • • • ]
        // 5 [ • • • • • • • • • • ]
        // 6 [ • • • • • • • • • • ]
        // 7 [ • • • • • • • • • • ]
        // 8 [ • • • • • • • • • • ]
        // 9 [ • • • • • • • • • • ]
        let flag_x = 5;
        let flag_y = 1;
        let toggle_result = minefield.toggle_flag(flag_x, flag_y);
        assert_eq!(toggle_result, FlagToggleResult::Added);

        // Step on spot (x=9, y=6)
        //     0 1 2 3 4 5 6 7 8 9
        // 0 [     1 • • • • • • • ]
        // 1 [     1 1 1 ⚐ • • • • ]
        // 2 [           1 • • • • ]
        // 3 [   1 1 1   1 • • • • ]
        // 4 [   1 • 1   1 1 1 1 • ]
        // 5 [   1 1 1         1 1 ]
        // 6 [         1 1 2 1 1   ]
        // 7 [         1 • • • 1   ]
        // 8 [         1 1 2 1 1   ]
        // 9 [                     ]
        let step_x = 9;
        let step_y = 6;
        let step_result = minefield.step(step_x, step_y);
        assert_eq!(step_result, StepResult::Phew);

        // All mines are still hidden
        for n_coords in mine_coords {
            assert_eq!(minefield.field.get(&n_coords).unwrap().state, SpotState::HiddenMine);
        }

        // Flood revealed half maze
        assert_eq!(minefield.field.get(&(7, 5)).unwrap().state, SpotState::RevealedEmpty { neighboring_mines: 0 });

        // Flag is still there
        assert_eq!(minefield.field.get(&(flag_x, flag_y)).unwrap().state, SpotState::FlaggedEmpty { neighboring_mines: 0 });

        // Insulated portion of field is still hidden
        assert_eq!(minefield.field.get(&(9, 0)).unwrap().state, SpotState::HiddenEmpty { neighboring_mines: 0 });
        assert_eq!(minefield.field.get(&(7, 1)).unwrap().state, SpotState::HiddenEmpty { neighboring_mines: 0 });
     }

     #[allow(dead_code)]
     fn print_minefield(minefield: &Minefield) {
        // X axis
        println!();
        print!("   ");
        for y in 0..minefield.width {
            print!(" {}", y);
        }
        println!();

        for y in 0..minefield.height {
            // Y Axis
            print!("{:?} [", y);
            for x in 0..minefield.width {
                match minefield.field.get(&(x, y)).unwrap().state {
                    SpotState::FlaggedMine | SpotState::HiddenMine | SpotState::ExplodedMine => {
                        print!(" ☢");
                    },
                    SpotState::FlaggedEmpty { neighboring_mines } | SpotState::HiddenEmpty { neighboring_mines } | SpotState::RevealedEmpty { neighboring_mines } => {
                        if neighboring_mines > 0 {
                            print!(" {}", neighboring_mines);
                        } else {
                            print!("  ");
                        }
                    },
                }
            }
            println!(" ]");
        }
     }

     #[allow(dead_code)]
     fn print_minefield_state(minefield: &Minefield) {
        // X axis
        println!();
        print!("   ");
        for y in 0..minefield.width {
            print!(" {}", y);
        }
        println!();

        for y in 0..minefield.height {
            // Y Axis
            print!("{:?} [", y);
            for x in 0..minefield.width {
                match minefield.field.get(&(x, y)).unwrap().state {
                    SpotState::HiddenEmpty { neighboring_mines: _ } => {
                        print!(" •");
                    },
                    SpotState::HiddenMine => {
                        print!(" •");
                    },
                    SpotState::FlaggedEmpty { neighboring_mines: _ } => {
                        print!(" ⚐");
                    },
                    SpotState::FlaggedMine => {
                        print!(" ⚐");
                    },
                    SpotState::RevealedEmpty { neighboring_mines } => {
                        if neighboring_mines > 0 {
                            print!(" {}", neighboring_mines);
                        } else {
                            print!("  ");
                        }
                    },
                    SpotState::ExplodedMine => {
                        print!(" 💥");
                    },
                }
            }
            println!(" ]");
        }
     }
 }