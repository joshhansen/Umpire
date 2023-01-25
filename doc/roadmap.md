# Umpire Roadmap

## 0.4 Milestones
* ~~TODO Refresh README.md~~
* ~~TODO Deploy to crates.io~~

## 0.5 Milestones
* TODO Client/server architecture
 - what's the implication for testing? Requires a network connection?
* TODO Fuel limits for aircraft
* TODO Wake up units with auto-explore and go-to orders when they encounter something interesting
* TODO Wake up sentried units when an enemy comes within their sight.
* TODO Opening theme music
* TODO Allow map specification at command-line
* FIXME Make it clear when a unit is inside a city
* TODO Initial AI framework
* TODO Make active unit blink---it's too hard to see right now.
* FIXME Small maps aren't centered properly (e.g. 10x10)
* FIXME Rendering issues if wrapping is off
* FIXME Explore and go-to moves aren't animating visibly
* FIXME Autoscale splashscreen to fit terminal dimensions

## 0.6 Milestones
* TODO Make splash screen respect color palette
* FIXME Make splash screen fit the terminal size
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

## Other

* FIXME Handle terminal resize events properly


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
