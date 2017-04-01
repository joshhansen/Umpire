// From http://stackoverflow.com/a/27590832/5374919
//FIXME Be on the lookout for a standard version of this so we can drop this custom macro ASAP
macro_rules! printerr(
    ($($arg:tt)*) => { {
        use std::io::Write;
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);
