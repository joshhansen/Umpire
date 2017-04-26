//! Name generation for units and cities.

use std::fmt::Debug;
use std::path::Path;
use std::ops::AddAssign;
use std::str::FromStr;

use csv;
use rand::{thread_rng, Rng, ThreadRng};
use rand::distributions::{IndependentSample, Range};
use rand::distributions::range::SampleRange;

/// Something that generates names.
pub trait Namer {
    fn name(&mut self) -> String;
}

/// Generate names by drawing from a predefined list.
pub struct ListNamer {
    names: Vec<String>,
    next_name: usize
}
impl ListNamer {
    fn new(names: Vec<String>) -> Self {
        ListNamer{names: names, next_name: 0}
    }
}
impl Namer for ListNamer {
    fn name(&mut self) -> String {
        let name = self.names[self.next_name % self.names.len()].clone();
        self.next_name += 1;
        name
    }
}

/// Generate names by sampling from a weighted distribution of names.
pub struct WeightedNamer<N:Default> {
    cumulatively_weighted_names: Vec<CumWeight<String,N>>,
    sample_range: Range<N>,
    rng: ThreadRng
}
impl <N: Copy+Default+PartialOrd+SampleRange> WeightedNamer<N> {
    pub fn new(cumulatively_weighted_names: Vec<CumWeight<String,N>>) -> Self {
        // let total_weight = weighted_names.iter().fold(0, |acc, &weighted| acc + weighted.weight);
        let zero: N = Default::default();
        let weight_range = Range::new(zero, cumulatively_weighted_names[cumulatively_weighted_names.len()-1].cum_weight);

        // let mut cumulatively_weighted_names
        // WeightedNamer {
        //     weighted_names: weighted_names,
        //     total_weight: total_weight
        // }
        // let choice = WeightedChoice::new(weighted_names);
        WeightedNamer {
            // weighted_names_dist: choice,
            // weighted_names: weighted_names,
            cumulatively_weighted_names: cumulatively_weighted_names,
            sample_range: weight_range,
            rng: thread_rng()
        }
    }
}
impl <N: Default+PartialOrd+SampleRange> Namer for WeightedNamer<N> {
    fn name(&mut self) -> String {
        // self.weighted_names_dist.ind_sample(&mut self.rng)
        let x = self.sample_range.ind_sample(&mut self.rng);
        for cumulatively_weighted_name in &self.cumulatively_weighted_names {
            if cumulatively_weighted_name.cum_weight >= x {
                return cumulatively_weighted_name.item.clone();
            }
        }
        panic!("In theory this code is unreachable. In practice, bugs happen.");
    }
}

fn shuffle(names: Vec<String>) -> Vec<String> {
    let mut names = names;
    let mut rng = thread_rng();
    rng.shuffle(&mut names);
    names
}

// fn load_list(filename: &'static str) -> std::io::Result<Vec<String>> {
//     let f = File::open(filename)?;
//     let f = BufReader::new(f);
//
//     let mut items = Vec::new();
//     for line in f.lines() {
//         items.push(line.unwrap());
//     }
//     Ok(items)
// }

pub struct CumWeight<T,N> {
    item: T,
    cum_weight: N
}

/// Load a CSV file with no header but the form $string,$weight
/// The strings and _cumulative_ weights are loaded into a vector.
/// Implemented generically to allow a variety of numeric types to be used for the weight
fn load_cumulative_weights<N>(filename: &'static str) -> Result<Vec<CumWeight<String,N>>,String>
    where N: AddAssign+Copy+Debug+Default+FromStr,
          N::Err: Debug {

    let path = Path::new(filename);
    csv::Reader::from_file(path)
    .map_err(|csv_err| format!("Error reading CSV from file {:?}: {}", path, csv_err))
    .map(|reader| {
        let mut weighted_names = Vec::new();
        let mut reader = reader.has_headers(false);
        let mut cumulative_weight: N = Default::default();
        for row in reader.records() {
            let row = row.unwrap();
            let weight = row[1].parse::<N>().unwrap();
            cumulative_weight += weight;
            weighted_names.push(CumWeight {
                cum_weight: cumulative_weight,
                item: row[0].clone()
            });
        }
        weighted_names
    })
}

/// From Geonames schema
static CITY_NAME_COL: usize = 1;

/// The default city namer.
///
/// Loads city names from the geonames 1000 cities database and returns them randomly.
pub fn city_namer() -> Result<ListNamer,String> {
    let path = Path::new("data/geonames_cities1000_2017-02-27_02:01.tsv");

    // match csv::Reader::from_file(path) {
    //     Ok(reader) => {},
    //     Err(csv_err) => Err(format!("Error reading CSV from file {}: {}", path, csv_err))
    // }

    csv::Reader::from_file(path)
    .map_err(|csv_err| format!("Error reading CSV from file {:?}: {}", path, csv_err))
    .map(|reader| {
        let mut names = Vec::new();
        let mut reader = reader.has_headers(false).delimiter(b'\t');
        for row in reader.records() {
            let row = row.unwrap();
            // println!("{}, {}: {}", n1, n2, dist);
            // println!("{:?}", row);
            names.push(row[CITY_NAME_COL].clone());
        }
        println!("Cities loaded.");
        ListNamer::new(shuffle(names))
    })

    // match csv::Reader::from_file(path) {
    //     Ok(reader) => {
    //         let mut names = Vec::new();
    //         let mut reader = reader.has_headers(false).delimiter(b'\t');
    //         for row in reader.records() {
    //             let row = row.unwrap();
    //             // println!("{}, {}: {}", n1, n2, dist);
    //             // println!("{:?}", row);
    //             names.push(row[CITY_NAME_COL].clone());
    //         }
    //         println!("Cities loaded.");
    //         Ok(ListNamer::new(shuffle(names)))
    //     },
    //     Err(err) => {
    //         return Err(format!("Error reading cities data file: {}", err));
    //     }
    // }
}

/// Generate names by deferring to two sub-namers and joining their output
pub struct CompoundNamer<N1:Namer,N2:Namer> {
    join_str: &'static str,
    namer1: N1,
    namer2: N2
}
impl <N1:Namer,N2:Namer> CompoundNamer<N1,N2> {
    fn new(join_str: &'static str, namer1: N1, namer2: N2) -> Self {
        CompoundNamer{
            join_str: join_str,
            namer1: namer1,
            namer2: namer2
        }
    }
}
impl<N1:Namer,N2:Namer> Namer for CompoundNamer<N1,N2> {
    fn name(&mut self) -> String {
        format!("{}{}{}", self.namer1.name(), self.join_str, self.namer2.name())
    }
}

/// The default unit namer.
///
/// Loads given names and surnames from census data and combines them randomly in accordance with
/// their prevalence in the American population.
pub fn unit_namer() -> Result<CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>, String> {
    let givennames = load_cumulative_weights("data/us-census/1990/givenname_rel_freqs.csv")?;
    let surnames = load_cumulative_weights("data/us-census/2010/surname_freqs.csv")?;
    Ok(CompoundNamer::new(
        " ",
        WeightedNamer::new(givennames),
        WeightedNamer::new(surnames)
    ))
}

#[allow(dead_code)]
pub fn test_unit_namer() -> Result<CompoundNamer<WeightedNamer<f64>,WeightedNamer<u32>>, String> {
    let givennames = load_cumulative_weights("data/us-census/1990/givenname_rel_freqs-test.csv")?;
    let surnames = load_cumulative_weights("data/us-census/2010/surname_freqs-test.csv")?;
    Ok(CompoundNamer::new(
        " ",
        WeightedNamer::new(givennames),
        WeightedNamer::new(surnames)
    ))
}
