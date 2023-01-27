use crate::game::PlayerType;

/// An item specified by a string on the command line
pub trait Specified: TryFrom<String> {
    /// A description to show up in the command line help
    fn desc(&self) -> String;

    /// A canonicalized string representation of the item
    fn spec(&self) -> String;
}

/// Parse a our little specification sub-language
///
/// Comma-separated and (for tokens that aren't file paths or otherwise already map to an item) split into individual
/// characters.
///
/// Examples:
/// * r
/// * r123
/// * r123,ai/model.ai,ai/tf_model
pub fn parse_spec<S1: AsRef<str>, S2: AsRef<str>, T: Specified>(
    spec: S1,
    spec_name: S2,
) -> Result<Vec<T>, String> {
    let mut items: Vec<T> = Vec::new();
    for spec2 in spec.as_ref().split(",") {
        match T::try_from(spec2.to_string()) {
            Ok(item) => items.push(item),
            Err(_) => {
                // char by char
                for spec3 in spec2.split_terminator("").skip(1) {
                    items.push(T::try_from(spec3.to_string()).map_err(|_| {
                        format!(
                            "{} is not a valid {} specification",
                            spec2,
                            spec_name.as_ref()
                        )
                    })?);
                }
            }
        }
    }
    Ok(items)
}

/// Parse the player specification
///
/// Examples:
/// * hr
/// * hr123
/// * hhhh
/// * hr123,ai/model.ai,ai/tf_model
pub fn parse_player_spec<S: AsRef<str>>(spec: S) -> Result<Vec<PlayerType>, String> {
    parse_spec(spec, "player")
}
