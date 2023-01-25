//! Name generation for units and cities.

use std::fmt::Debug;
use std::ops::AddAssign;
use std::str::FromStr;

use csv;
use flate2::read::GzDecoder;

use rand::{
    distributions::{
        uniform::{SampleUniform, Uniform},
        Distribution,
    },
    prelude::SliceRandom,
    thread_rng,
};

/// Something that generates names.
pub trait Namer: Send + Sync {
    fn name(&mut self) -> String;
}

/// Something that has a name
pub trait Named {
    fn name(&self) -> &String;
}

/// A namer that names things after numbers, counting upward from zero. A prefix is prepended.
pub struct IntNamer {
    next: u64,
    prefix: String,
}
impl IntNamer {
    pub fn new<S: ToString>(prefix: S) -> Self {
        Self {
            prefix: prefix.to_string(),
            next: 0,
        }
    }
}
impl Namer for IntNamer {
    fn name(&mut self) -> String {
        let name = format!("{}{}", self.prefix, self.next);
        self.next += 1;
        name
    }
}

/// Generate names by drawing from a predefined list.
pub struct ListNamer {
    names: Vec<String>,
    next_name: usize,
}
impl ListNamer {
    fn new(names: Vec<String>) -> Self {
        ListNamer {
            names,
            next_name: 0,
        }
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
pub struct WeightedNamer<N: Default + SampleUniform> {
    cumulatively_weighted_names: Vec<CumWeight<String, N>>,
    // sample_range: Uniform<N>,
    // rng: ThreadRng
}
impl<N: Copy + Default + PartialOrd + SampleUniform> WeightedNamer<N> {
    pub fn new(cumulatively_weighted_names: Vec<CumWeight<String, N>>) -> Self {
        // let total_weight = weighted_names.iter().fold(0, |acc, &weighted| acc + weighted.weight);
        // let zero: N = Default::default();
        // let weight_range = Uniform::new_inclusive(zero, cumulatively_weighted_names[cumulatively_weighted_names.len()-1].cum_weight);

        // let mut cumulatively_weighted_names
        // WeightedNamer {
        //     weighted_names: weighted_names,
        //     total_weight: total_weight
        // }
        // let choice = WeightedChoice::new(weighted_names);
        WeightedNamer {
            // weighted_names_dist: choice,
            // weighted_names: weighted_names,
            cumulatively_weighted_names,
            // sample_range: weight_range,
            // rng: thread_rng()
        }
    }
}
impl<N: Copy + Default + PartialOrd + SampleUniform + Send + Sync> Namer for WeightedNamer<N> {
    fn name(&mut self) -> String {
        // self.weighted_names_dist.ind_sample(&mut self.rng)
        let zero: N = Default::default();
        let sample_range = Uniform::new_inclusive(
            zero,
            self.cumulatively_weighted_names[self.cumulatively_weighted_names.len() - 1].cum_weight,
        );
        let x = sample_range.sample(&mut rand::thread_rng());
        for cumulatively_weighted_name in &self.cumulatively_weighted_names {
            if cumulatively_weighted_name.cum_weight >= x {
                return cumulatively_weighted_name.item.clone();
            }
        }
        unreachable!("In theory this code is unreachable. In practice, bugs happen.");
    }
}

fn shuffle(names: Vec<String>) -> Vec<String> {
    let mut names = names;
    let mut rng = thread_rng();
    names.shuffle(&mut rng);
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

pub struct CumWeight<T, N> {
    item: T,
    cum_weight: N,
}

/// Load a CSV file with no header but the form $string,$weight
/// The strings and _cumulative_ weights are loaded into a vector.
/// Implemented generically to allow a variety of numeric types to be used for the weight
fn load_cumulative_weights<N>(bytes: &[u8]) -> Vec<CumWeight<String, N>>
where
    N: AddAssign + Copy + Debug + Default + FromStr,
    N::Err: Debug,
{
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(bytes);

    let mut weighted_names = Vec::new();
    let mut cumulative_weight: N = Default::default();
    for row in reader.records() {
        let row: csv::StringRecord = row.unwrap();
        let weight = row[1].parse::<N>().unwrap();
        cumulative_weight += weight;
        weighted_names.push(CumWeight {
            cum_weight: cumulative_weight,
            item: row[0].into(),
        });
    }
    weighted_names
}

/// From Geonames schema
static CITY_NAME_COL: usize = 1;

/// The default city namer.
///
/// Loads city names from the geonames 1000 cities database and returns them randomly.
pub fn city_namer() -> ListNamer {
    let bytes_gz: &[u8] =
        include_bytes!("../data/geonames_cities1000_2017-02-27_0201__pop-and-name.tsv.gz");
    let d = GzDecoder::new(bytes_gz);

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .from_reader(d);

    let mut names = Vec::new();
    for row in reader.records() {
        let row = row.unwrap();
        names.push(row[CITY_NAME_COL].into());
    }

    ListNamer::new(shuffle(names))
}

/// Generate names by deferring to two sub-namers and joining their output
pub struct CompoundNamer<N1: Namer, N2: Namer> {
    join_str: &'static str,
    namer1: N1,
    namer2: N2,
}
impl<N1: Namer, N2: Namer> CompoundNamer<N1, N2> {
    fn new(join_str: &'static str, namer1: N1, namer2: N2) -> Self {
        CompoundNamer {
            join_str,
            namer1,
            namer2,
        }
    }
}
impl<N1: Namer, N2: Namer> Namer for CompoundNamer<N1, N2> {
    fn name(&mut self) -> String {
        format!(
            "{}{}{}",
            self.namer1.name(),
            self.join_str,
            self.namer2.name()
        )
    }
}

/// The default unit namer.
///
/// Loads given names and surnames from census data and combines them randomly in accordance with
/// their prevalence in the American population.
pub fn unit_namer() -> CompoundNamer<WeightedNamer<f64>, WeightedNamer<u32>> {
    let givenname_bytes: &[u8] = include_bytes!("../data/us-census/1990/givenname_rel_freqs.csv");
    let givennames = load_cumulative_weights(givenname_bytes);

    let surname_bytes: &[u8] = include_bytes!("../data/us-census/2010/surname_freqs.csv");
    let surnames = load_cumulative_weights(surname_bytes);
    CompoundNamer::new(
        " ",
        WeightedNamer::new(givennames),
        WeightedNamer::new(surnames),
    )
}
