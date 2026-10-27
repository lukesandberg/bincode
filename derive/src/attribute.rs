use virtue::prelude::*;
use virtue::utils::{parse_tagged_attribute, ParsedAttribute};

pub struct ContainerAttributes {
    pub crate_name: String,
    pub bounds: Option<(String, Literal)>,
    pub decode_bounds: Option<(String, Literal)>,
    pub decode_context: Option<(String, Literal)>,
    pub borrow_decode_bounds: Option<(String, Literal)>,
    pub encode_bounds: Option<(String, Literal)>,
    /// `#[bincode(no_niche)]`: don't declare niches, which wrappers like `Option` would otherwise
    /// use in place of a tag when the config enables them.
    pub no_niche: bool,
}

impl Default for ContainerAttributes {
    fn default() -> Self {
        Self {
            crate_name: "::bincode".to_string(),
            bounds: None,
            decode_bounds: None,
            decode_context: None,
            encode_bounds: None,
            borrow_decode_bounds: None,
            no_niche: false,
        }
    }
}

impl FromAttribute for ContainerAttributes {
    fn parse(group: &Group) -> Result<Option<Self>> {
        let attributes = match parse_tagged_attribute(group, "bincode")? {
            Some(body) => body,
            None => return Ok(None),
        };
        let mut result = Self::default();
        for attribute in attributes {
            match attribute {
                ParsedAttribute::Property(key, val) if key.to_string() == "crate" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.crate_name = val_string[1..val_string.len() - 1].to_string();
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "bounds" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.bounds =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "decode_bounds" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.decode_bounds =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "decode_context" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.decode_context =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "encode_bounds" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.encode_bounds =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val)
                    if key.to_string() == "borrow_decode_bounds" =>
                {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.borrow_decode_bounds =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Tag(i) if i.to_string() == "no_niche" => {
                    result.no_niche = true;
                }
                ParsedAttribute::Tag(i) => {
                    return Err(Error::custom_at("Unknown field attribute", i.span()))
                }
                ParsedAttribute::Property(key, _) => {
                    return Err(Error::custom_at("Unknown field attribute", key.span()))
                }
                _ => {}
            }
        }
        Ok(Some(result))
    }
}

#[derive(Default)]
pub struct FieldAttributes {
    pub with_serde: bool,
    pub skip: bool,
    pub default_fn: Option<(String, Literal)>,
    pub with: Option<String>,
}

impl FromAttribute for FieldAttributes {
    fn parse(group: &Group) -> Result<Option<Self>> {
        let attributes = match parse_tagged_attribute(group, "bincode")? {
            Some(body) => body,
            None => return Ok(None),
        };
        let mut default_span = None;
        let mut result = Self::default();
        for attribute in attributes {
            match attribute {
                ParsedAttribute::Tag(i) if i.to_string() == "with_serde" => {
                    if result.skip {
                        return Err(mutually_exclusive_err(&i, "skip"));
                    } else if result.with.is_some() {
                        return Err(mutually_exclusive_err(&i, "with"));
                    }
                    result.with_serde = true;
                }
                ParsedAttribute::Tag(i) if i.to_string() == "skip" => {
                    if result.with_serde {
                        return Err(mutually_exclusive_err(&i, "with_serde"));
                    } else if result.with.is_some() {
                        return Err(mutually_exclusive_err(&i, "with"));
                    }
                    result.skip = true;
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "default" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.default_fn =
                            Some((val_string[1..val_string.len() - 1].to_string(), val));
                        default_span = Some(key.span());
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(key, val) if key.to_string() == "with" => {
                    if result.skip {
                        return Err(mutually_exclusive_err(&key, "skip"));
                    } else if result.with_serde {
                        return Err(mutually_exclusive_err(&key, "with_serde"));
                    }
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.with = Some(val_string[1..val_string.len() - 1].to_string());
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Tag(i) => {
                    return Err(Error::custom_at("Unknown field attribute", i.span()))
                }
                ParsedAttribute::Property(key, _) => {
                    return Err(Error::custom_at("Unknown field attribute", key.span()))
                }
                _ => {}
            }
        }
        if let Some(default_span) = default_span {
            if !result.skip {
                return Err(Error::custom_at(
                    "`default` attribute requires `skip`, but `skip` was not found",
                    default_span,
                ));
            }
        }
        Ok(Some(result))
    }
}

#[derive(Default)]
pub struct VariantAttributes {
    pub with: String,
}

impl FromAttribute for VariantAttributes {
    fn parse(group: &Group) -> Result<Option<Self>> {
        let attributes = match parse_tagged_attribute(group, "bincode")? {
            Some(body) => body,
            None => return Ok(None),
        };
        let mut result = Self::default();
        for attribute in attributes {
            match attribute {
                ParsedAttribute::Property(key, val) if key.to_string() == "with" => {
                    let val_string = val.to_string();
                    if val_string.starts_with('"') && val_string.ends_with('"') {
                        result.with = val_string[1..val_string.len() - 1].to_string();
                        if result.with.is_empty() {
                            return Err(Error::custom_at("Should not be empty", val.span()));
                        }
                    } else {
                        return Err(Error::custom_at("Should be a literal str", val.span()));
                    }
                }
                ParsedAttribute::Property(i, _) | ParsedAttribute::Tag(i) => {
                    return Err(Error::custom_at("Unknown field attribute", i.span()))
                }
                _ => {}
            }
        }
        if result.with.is_empty() {
            return Err(Error::custom_at("Missing `with` field", group.span()));
        }
        Ok(Some(result))
    }
}

fn mutually_exclusive_err(ident: &Ident, other: &str) -> Error {
    Error::custom_at(
        format!("Attribute {ident} is mutually exclusive with {other}"),
        ident.span(),
    )
}
