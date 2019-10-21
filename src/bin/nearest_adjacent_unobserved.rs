use std::io::{Write,stdout};

/// A function that is opaque to the optimizer, used to prevent the compiler from
/// optimizing away computations in a benchmark.
///
/// This variant is stable-compatible, but it may cause some performance overhead
/// or fail to prevent code from being eliminated.
#[cfg(not(feature = "real_blackbox"))]
pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

use umpire::{
    game::{
        Alignment,
        map::{
            LocationGrid,
            Tile,
            dijkstra::{
                nearest_adjacent_unobserved_reachable_without_attacking,
            },
            terrain::Terrain,
        },
        obs::Obs,
        unit::{UnitID,Unit,UnitType},
    },
    
    util::{Dims,Location,Wrap2d},
};

fn bench_nearest_adjacent_unobserved_reachable_without_attacking(dims: Dims, src: Location, wrapping: Wrap2d) {
    
    let dest = Location::new(dims.width-1, dims.height-1);

    let grid = LocationGrid::new(dims, |loc| {
        if loc==dest {
            Obs::Unobserved
        } else {
            Obs::Observed {
                tile: Tile::new(Terrain::Land, loc),
                turn: 0,
                current: false
            }
        }
    });

    let unit = Unit::new(UnitID::new(0), src, UnitType::Infantry, Alignment::Belligerent{player: 0}, "Juan de Fuca");
    let nearest = nearest_adjacent_unobserved_reachable_without_attacking(&grid, src, &unit, wrapping);
    black_box(nearest);
}

const ITERATIONS: usize = 1000;

fn main() {
    let src = Location::new(0, 0);
    let dims = Dims::new(11, 11);
    let (wrap_name,wrapping) = ("horiz", Wrap2d::HORIZ);
    // for src in [Location::new(0, 0), Location::new(4, 4)].iter() {
    //     for dims in [Dims::new(10, 10), Dims::new(11, 11)].iter() {
            // for (wrap_name,wrapping) in [
            //     // ("both",Wrap2d::BOTH),
            //     ("horiz",Wrap2d::HORIZ),
            //     ("vert",Wrap2d::VERT),
            //     ("neither",Wrap2d::NEITHER)
            // ].iter() {
                
                for it in 0..ITERATIONS {
                    bench_nearest_adjacent_unobserved_reachable_without_attacking(dims, src, wrapping);
                    if it % 1000 == 0 {
                        print!("\rsrc={} {} {} {}/{}     ", src, dims, wrap_name, it, ITERATIONS);
                        stdout().flush().unwrap();
                    }
                }

            // }
    //     }
    // }
}