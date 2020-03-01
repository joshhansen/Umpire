# Umpire Roadmap

## 0.4 Milestones
* FIXME Orders aren't being sticky for some reason
* FIXME Auto-explore mode for units is broken
  - it happens that the auto-explore will request a move that exceeds a unit's moves remaining
* TODO Allow examine mode go-to to unobserved tiles, on a best-effort basis
* TODO Refresh README.md
* TODO Deploy to crates.io

## 0.5 Milestones
* TODO Upgrade crossterm dependency to the most recent version
* TODO Fuel limits for aircraft
* TODO Wake up units with auto-explore and go-to orders when they encounter something interesting
* TODO Wake up sentried units when an enemy comes within their sight.
* TODO Opening theme music
* TODO Allow map specification at command-line
* FIXME Make it clear when a unit is inside a city
* TODO Initial AI framework

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


## Deferred


## Ideas
* AI
* Unit identities
 x Name
 - Nationality
 - Strengths/weaknesses
* City identities
 x Name
 - Nationality
 - Strengths/weaknesses
* Unit experience
* Random events
* UI themes: colors, symbols, maybe layouts
* Name generation: use a generative model to produce novel names for
  cities, units, nations, etc. See https://github.com/Tw1ddle/MarkovNameGenerator for ideas
