# Umpire Roadmap

## 0.4 Milestones
* FIXME Auto-explore mode for units is broken
  - it happens that the auto-explore will request a move that exceeds a unit's moves remaining
* FIXME Don't let aircraft or naval vessels conquer cities
* TODO Allow examine mode go-to to unobserved tiles, on a best-effort basis
* TODO Deploy to crates.io
* TODO Refresh README.md

## 0.5 Milestones
* TODO Fuel limits for aircraft
* TODO Wake up units with auto-explore and go-to orders when they encounter something interesting
* TODO Wake up sentried units when an enemy comes within their sight.
* TODO Opening theme music
* TODO Allow map specification at command-line
* FIXME Make it clear when a unit is inside a city

## 0.6 Milestones
* TODO Make splash screen respect color palette
* FIXME Make splash screen fit the terminal size
* FIXME Fix problems with small map sizes: 1) crash if viewport is larger than map 2) limitless wrapping where we should only wrap once
* TODO Zoomed-out map view?
* TODO Color console text announcing turn start to correspond to player colors?

## 0.7 Milestones
* TODO Travis infrastructure
* TODO Remove all git-based dependencies
* TODO Decruftification
* TODO API cleanup
* TODO Improved test coverage
* TODO? Move `log` into `ui` and make `Game` fully abstract?
* TODO Profile and optimize

## 0.8 Milestones
* TODO Windows support
* TODO OSX support
* TODO Game save/load

## 0.9 Milestones
* TODO Unit names that better reflect current world naming patterns rather than just the US from 10/20 years ago.

## 1.0 Milestones
* TODO AI