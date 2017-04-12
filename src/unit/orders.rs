use util::Location;

#[derive(Clone,Debug,PartialEq)]
pub enum Orders {
    Sentry,
    GoTo{dest:Location},
    Explore
}
