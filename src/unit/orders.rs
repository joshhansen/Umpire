use util::Location;

#[derive(Clone,Debug,PartialEq)]
pub enum Orders {
    Sentry,
    GoTo{loc:Location},
    Explore
}
