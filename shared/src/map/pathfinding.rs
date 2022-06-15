
//! # `BaseMap`
//!
//! `BaseMap` provides map traits required for path-finding and field-of-view operations. Implement these
//! if you want to use these features from `bracket-lib`.
//!
//! `is_opaque` specifies is you can see through a tile, required for field-of-view.
//!
//! `get_available_exits` lists the indices to which one can travel from a given tile, along with a relative
//! cost of each exit. Required for path-finding.
//!
//! `get_pathing_distance` allows you to implement your heuristic for determining remaining distance to a
//! target.
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use super::Point2;
use smallvec::SmallVec;

/// Implement this trait to support path-finding functions.
pub trait BaseMap {
    /// Return a vector of tile indices to which one can path from the idx.
    /// These do NOT have to be contiguous - if you want to support teleport pads, that's awesome.
    /// Default implementation is provided that proves an empty list, in case you aren't using
    /// it.
    ///
    /// Note that you should never return the current tile as an exit. The A* implementation
    /// really doesn't like that.
    /// 
    /// modified, all neighbor node cost is 1
    fn get_available_exits(&self, _: Point2) -> SmallVec<[Point2; 6]> {
        SmallVec::new()
    }

    /// Return the distance you would like to use for path-finding. Generally, Pythagoras distance (implemented in geometry)
    /// is fine, but you might use Manhattan or any other heuristic that fits your problem.
    /// Default implementation returns 1.0, which isn't what you want but prevents you from
    /// having to implement it when not using it.
    fn distance_to_end(&self, _point: &Point2) -> u32 {
        1
    }
    ///check if target point is valid
    fn success_check(&self, _point: &Point2) -> bool{ true }
}
/// Bail out if the A* search exceeds this many steps.
const MAX_ASTAR_STEPS: usize = 65536;

/// Request an A-Star search. The start and end are specified as index numbers (compatible with your
/// BaseMap implementation), and it requires access to your map so as to call distance and exit determinations.
pub fn a_star_search<F: Fn(&Point2) -> bool>(start: Point2, predicter: &F, map: &dyn BaseMap) -> NavigationPath
{
    AStar::new(start).search(predicter, map)
}

/// Holds the result of an A-Star navigation query.
/// `destination` is the index of the target tile.
/// `success` is true if it reached the target, false otherwise.
/// `steps` is a vector of each step towards the target, *including* the starting position.
#[derive(Clone, Default)]
pub struct NavigationPath {
    pub destination: Point2,
    pub success: bool,
    pub steps: Vec<Point2>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
/// Node is an internal step inside the A-Star path (not exposed/public). Idx is the current cell,
/// f is the total cost, g the neighbor cost, and h the heuristic cost.
/// See: https://en.wikipedia.org/wiki/A*_search_algorithm
struct Node {
    pos: Point2,
    f: f32,
    g: f32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
    }
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, b: &Self) -> Ordering {
        b.f.partial_cmp(&self.f).unwrap()
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        b.f.partial_cmp(&self.f)
    }
}

impl NavigationPath {
    /// Makes a new (empty) NavigationPath
    pub fn new() -> NavigationPath {
        NavigationPath {
            destination: Default::default(),
            success: false,
            steps: Vec::new(),
        }
    }
}

/// Private structure for calculating an A-Star navigation path.
struct AStar {
    start: Point2,
    open_list: BinaryHeap<Node>,
    closed_list: HashMap<Point2, f32>,
    parents: HashMap<Point2, (Point2, f32)>, // (index, cost)
    step_counter: usize,
}

impl AStar {
    /// Creates a new path, with specified starting and ending indices.
    fn new(start: Point2) -> AStar {
        let mut open_list: BinaryHeap<Node> = BinaryHeap::new();
        open_list.push(Node {
          pos: start,
            f: 0.0,
            g: 0.0,
        });

        AStar {
            start,
            open_list,
            parents: HashMap::new(),
            closed_list: HashMap::new(),
            step_counter: 0,
        }
    }

    /// Wrapper to the BaseMap's distance function.
    fn distance_to_end(&self, point: &Point2, map: &dyn BaseMap) -> u32 {
        map.distance_to_end(point)
    }

    /// Adds a successor; if we're at the end, marks success.
    fn add_successor(&mut self, q: Node, pos: &Point2, cost: f32, map: &dyn BaseMap) {
        let distance = self.distance_to_end(pos, map);
        let s = Node {
          pos: *pos,
            f: distance as f32 + cost,
            g: cost,
        };

        // If a node with the same position as successor is in the open list with a lower f, skip add
        let mut should_add = true;
        if let Some(e) = self.parents.get(pos) {
            if e.1 < s.f {
                should_add = false;
            }
        }

        // If a node with the same position as successor is in the closed list, with a lower f, skip add
        if should_add && self.closed_list.contains_key(pos) {
            should_add = false;
        }

        if should_add {
            self.open_list.push(s);
            self.parents.insert(*pos, (q.pos, q.f));
        }
    }

    /// Helper function to unwrap a path once we've found the end-point.
    fn found_it(&self, end: Point2) -> NavigationPath {
        let mut result = NavigationPath::new();
        result.success = true;
        result.destination = end;

        result.steps.push(end);
        let mut current = end;
        while current != self.start {
            let parent = self.parents[&current];
            result.steps.insert(0, parent.0);
            current = parent.0;
        }

        result
    }

    /// Performs an A-Star search
    fn search<F: Fn(&Point2) -> bool>(&mut self, predicter: &F, map: &dyn BaseMap) -> NavigationPath {
        let result = NavigationPath::new();
        while !self.open_list.is_empty() && self.step_counter < MAX_ASTAR_STEPS {
            self.step_counter += 1;

            // Pop Q off of the list
            let q = self.open_list.pop().unwrap();
            if predicter(&q.pos) {
                let success = self.found_it(q.pos);
                return success;
            }

            // Generate successors
            map.get_available_exits(q.pos)
                .iter()
                .for_each(|s| self.add_successor(q, s, 1.0 + q.f, map));

            if self.closed_list.contains_key(&q.pos) {
                self.closed_list.remove(&q.pos);
            }
            self.closed_list.insert(q.pos, q.f);
        }
        result
    }
}