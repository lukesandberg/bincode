use crate::attribute::{ContainerAttributes, FieldAttributes};
use virtue::parse::{IdentOrIndex, UnnamedField};
use virtue::prelude::*;

pub(crate) struct DeriveStruct {
    pub fields: Option<Fields>,
    pub attributes: ContainerAttributes,
}

impl DeriveStruct {
    /// The field whose niches the struct takes: its first encoded field, if that field uses its
    /// own `Encode` and `Decode` impls and its type has no lifetimes. Returns its name and type,
    /// or `None` if there is no such field or the struct has `#[bincode(no_niche)]`.
    ///
    /// The niches are named by the field's type, as in `<Cow<'a, str> as BorrowDecode<'__de, _>>`,
    /// which may not hold for the lifetimes as written even though the decoded field does, so
    /// fields with lifetimes are left out. All three derives apply the same rule so that they
    /// agree on the niches.
    fn niche_field(&self) -> Result<Option<(IdentOrIndex, String)>> {
        if self.attributes.no_niche {
            return Ok(None);
        }
        let fields: Vec<&UnnamedField> = match &self.fields {
            Some(Fields::Tuple(fields)) => fields.iter().collect(),
            Some(Fields::Struct(fields)) => fields.iter().map(|(_, field)| field).collect(),
            None => Vec::new(),
        };
        let names = self.fields.as_ref().map(Fields::names).unwrap_or_default();
        for (name, field) in names.into_iter().zip(fields) {
            let attributes = name
                .attributes()
                .get_attribute::<FieldAttributes>()?
                .unwrap_or_default();
            if attributes.skip {
                continue;
            }
            if attributes.with.is_some() || attributes.with_serde || has_lifetime(&field.r#type) {
                return Ok(None);
            }
            let ty = TokenStream::from_iter(field.r#type.iter().cloned()).to_string();
            return Ok(Some((name, ty)));
        }
        Ok(None)
    }

    pub fn generate_encode(self, generator: &mut Generator) -> Result<()> {
        let crate_name = &self.attributes.crate_name;
        let niche_field = self.niche_field()?;
        let mut impl_for = generator.impl_for(format!("{}::Encode", crate_name));
        impl_for
            .modify_generic_constraints(|generics, where_constraints| {
                if let Some((bounds, lit)) =
                    (self.attributes.encode_bounds.as_ref()).or(self.attributes.bounds.as_ref())
                {
                    where_constraints.clear();
                    where_constraints
                        .push_parsed_constraint(bounds)
                        .map_err(|e| e.with_span(lit.span()))?;
                } else {
                    for g in generics.iter_generics() {
                        where_constraints
                            .push_constraint(g, format!("{}::Encode", crate_name))
                            .unwrap();
                    }
                }
                Ok(())
            })?
            .generate_fn("encode")
            .with_generic_deps("__E", [format!("{}::enc::Encoder", crate_name)])
            .with_self_arg(virtue::generate::FnSelfArg::RefSelf)
            .with_arg("encoder", "&mut __E")
            .with_return_type(format!(
                "core::result::Result<(), {}::error::EncodeError>",
                crate_name
            ))
            .body(|fn_body| {
                if let Some(fields) = self.fields.as_ref() {
                    for field in fields.names() {
                        let attributes = field
                            .attributes()
                            .get_attribute::<FieldAttributes>()?
                            .unwrap_or_default();
                        if attributes.skip {
                            continue;
                        } else if let Some(with) = attributes.with {
                            fn_body.push_parsed(format!(
                                "{}::encode(&self.{}, encoder)?;",
                                with, field
                            ))?;
                        } else if attributes.with_serde {
                            fn_body.push_parsed(format!(
                                "{0}::Encode::encode(&{0}::serde::Compat(&self.{1}), encoder)?;",
                                crate_name, field
                            ))?;
                        } else {
                            fn_body.push_parsed(format!(
                                "{}::Encode::encode(&self.{}, encoder)?;",
                                crate_name, field
                            ))?;
                        }
                    }
                }
                fn_body.push_parsed("core::result::Result::Ok(())")?;
                Ok(())
            })?;
        if let Some((_, ty)) = &niche_field {
            impl_for
                .generate_const(
                    "ENCODE_NICHES",
                    format!("{}::niche::NicheTable", crate_name),
                )
                .with_value(|value| {
                    value.push_parsed(format!(
                        "<{} as {}::Encode>::ENCODE_NICHES",
                        ty, crate_name
                    ))?;
                    Ok(())
                })?;
        }
        Ok(())
    }

    pub fn generate_decode(self, generator: &mut Generator) -> Result<()> {
        // Remember to keep this mostly in sync with generate_borrow_decode
        let crate_name = &self.attributes.crate_name;
        let decode_context = if let Some((decode_context, _)) = &self.attributes.decode_context {
            decode_context.as_str()
        } else {
            "__Context"
        };
        let niche_field = self.niche_field()?;

        {
            let mut impl_for = generator.impl_for(format!("{}::Decode", crate_name));
            if self.attributes.decode_context.is_none() {
                impl_for = impl_for.with_impl_generics(["__Context"]);
            }
            let mut impl_for = impl_for.with_trait_generics([decode_context]);

            impl_for
                .modify_generic_constraints(|generics, where_constraints| {
                    if let Some((bounds, lit)) =
                        (self.attributes.decode_bounds.as_ref()).or(self.attributes.bounds.as_ref())
                    {
                        where_constraints.clear();
                        where_constraints
                            .push_parsed_constraint(bounds)
                            .map_err(|e| e.with_span(lit.span()))?;
                    } else {
                        for g in generics.iter_generics() {
                            where_constraints
                                .push_constraint(
                                    g,
                                    format!("{}::Decode<{}>", crate_name, decode_context),
                                )
                                .unwrap();
                        }
                    }
                    Ok(())
                })?
                .generate_fn("decode")
                .with_generic_deps(
                    "__D",
                    [format!(
                        "{}::de::Decoder<Context = {}>",
                        crate_name, decode_context
                    )],
                )
                .with_arg("decoder", "&mut __D")
                .with_return_type(format!(
                    "core::result::Result<Self, {}::error::DecodeError>",
                    crate_name
                ))
                .body(|fn_body| self.decode_fields(fn_body, decode_context, false))?;

            if let Some((_, ty)) = &niche_field {
                impl_for
                    .generate_const(
                        "DECODE_NICHES",
                        format!("{}::niche::NicheTable", crate_name),
                    )
                    .with_value(|value| {
                        value.push_parsed(format!(
                            "<{} as {}::Decode<{}>>::DECODE_NICHES",
                            ty, crate_name, decode_context
                        ))?;
                        Ok(())
                    })?;
                impl_for
                    .generate_fn("decode_with_first_byte")
                    .with_generic_deps(
                        "__D",
                        [format!(
                            "{}::de::Decoder<Context = {}>",
                            crate_name, decode_context
                        )],
                    )
                    .with_arg("__first", "u8")
                    .with_arg("decoder", "&mut __D")
                    .with_return_type(format!(
                        "core::result::Result<Self, {}::error::DecodeError>",
                        crate_name
                    ))
                    .body(|fn_body| self.decode_fields(fn_body, decode_context, true))?;
            }
        }
        self.generate_borrow_decode(generator)?;
        Ok(())
    }

    /// Generates `Ok(Self { .. })`, decoding each field in turn. With `first_byte`, the first
    /// encoded field is decoded from the byte in `__first`, which has already been read.
    fn decode_fields(
        &self,
        fn_body: &mut StreamBuilder,
        decode_context: &str,
        first_byte: bool,
    ) -> Result {
        let crate_name = &self.attributes.crate_name;
        let mut first_byte = first_byte;
        // Ok(Self {
        fn_body.push_parsed("core::result::Result::Ok")?;
        fn_body.group(Delimiter::Parenthesis, |ok_group| {
            ok_group.ident_str("Self");
            ok_group.group(Delimiter::Brace, |struct_body| {
                // Fields
                // {
                //      a: bincode::Decode::decode(decoder)?,
                //      b: bincode::Decode::decode(decoder)?,
                //      ...
                // }
                if let Some(fields) = self.fields.as_ref() {
                    for field in fields.names() {
                        let attributes = field
                            .attributes()
                            .get_attribute::<FieldAttributes>()?
                            .unwrap_or_default();
                        if attributes.skip {
                            if let Some((default_fn, _)) = attributes.default_fn {
                                struct_body.push_parsed(format!("{}: ({})(),", field, default_fn))?;
                            } else {
                                struct_body.push_parsed(format!(
                                    "{}: core::default::Default::default(),",
                                    field,
                                ))?;
                            }
                        } else if first_byte {
                            // `niche_field` ensures this field uses its own `Decode` impl.
                            first_byte = false;
                            struct_body.push_parsed(format!(
                                "{1}: {0}::Decode::decode_with_first_byte(__first, decoder)?,",
                                crate_name, field
                            ))?;
                        } else if let Some(with) = attributes.with {
                            struct_body.push_parsed(format!("{}: {}::decode(decoder)?,", field, with))?;
                        } else if attributes.with_serde {
                            struct_body.push_parsed(format!(
                                "{1}: (<{0}::serde::Compat<_> as {0}::Decode::<{2}>>::decode(decoder)?).0,",
                                crate_name, field, decode_context,
                            ))?;
                        } else {
                            struct_body.push_parsed(format!(
                                "{1}: {0}::Decode::decode(decoder)?,",
                                crate_name, field
                            ))?;
                        }
                    }
                }
                Ok(())
            })?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn generate_borrow_decode(self, generator: &mut Generator) -> Result<()> {
        // Remember to keep this mostly in sync with generate_decode
        let crate_name = &self.attributes.crate_name;

        let decode_context = if let Some((decode_context, _)) = &self.attributes.decode_context {
            decode_context.as_str()
        } else {
            "__Context"
        };
        let niche_field = self.niche_field()?;

        let mut impl_for = generator
            .impl_for_with_lifetimes(format!("{}::BorrowDecode", crate_name), ["__de"])
            .with_trait_generics([decode_context]);
        if self.attributes.decode_context.is_none() {
            impl_for = impl_for.with_impl_generics(["__Context"]);
        }

        impl_for
            .modify_generic_constraints(|generics, where_constraints| {
                if let Some((bounds, lit)) = (self.attributes.borrow_decode_bounds.as_ref())
                    .or(self.attributes.bounds.as_ref())
                {
                    where_constraints.clear();
                    where_constraints
                        .push_parsed_constraint(bounds)
                        .map_err(|e| e.with_span(lit.span()))?;
                } else {
                    for g in generics.iter_generics() {
                        where_constraints
                            .push_constraint(
                                g,
                                format!(
                                    "{}::de::BorrowDecode<'__de, {}>",
                                    crate_name, decode_context
                                ),
                            )
                            .unwrap();
                    }
                    for lt in generics.iter_lifetimes() {
                        where_constraints
                            .push_parsed_constraint(format!("'__de: '{}", lt.ident))?;
                    }
                }
                Ok(())
            })?
            .generate_fn("borrow_decode")
            .with_generic_deps(
                "__D",
                [format!(
                    "{}::de::BorrowDecoder<'__de, Context = {}>",
                    crate_name, decode_context
                )],
            )
            .with_arg("decoder", "&mut __D")
            .with_return_type(format!(
                "core::result::Result<Self, {}::error::DecodeError>",
                crate_name
            ))
            .body(|fn_body| self.borrow_decode_fields(fn_body, decode_context, false))?;

        if let Some((_, ty)) = &niche_field {
            impl_for
                .generate_const(
                    "BORROW_DECODE_NICHES",
                    format!("{}::niche::NicheTable", crate_name),
                )
                .with_value(|value| {
                    value.push_parsed(format!(
                        "<{} as {}::BorrowDecode<'__de, {}>>::BORROW_DECODE_NICHES",
                        ty, crate_name, decode_context
                    ))?;
                    Ok(())
                })?;
            impl_for
                .generate_fn("borrow_decode_with_first_byte")
                .with_generic_deps(
                    "__D",
                    [format!(
                        "{}::de::BorrowDecoder<'__de, Context = {}>",
                        crate_name, decode_context
                    )],
                )
                .with_arg("__first", "u8")
                .with_arg("decoder", "&mut __D")
                .with_return_type(format!(
                    "core::result::Result<Self, {}::error::DecodeError>",
                    crate_name
                ))
                .body(|fn_body| self.borrow_decode_fields(fn_body, decode_context, true))?;
        }
        Ok(())
    }

    /// The borrowing counterpart of [`decode_fields`](Self::decode_fields).
    fn borrow_decode_fields(
        &self,
        fn_body: &mut StreamBuilder,
        decode_context: &str,
        first_byte: bool,
    ) -> Result {
        let crate_name = &self.attributes.crate_name;
        let mut first_byte = first_byte;
        fn_body.push_parsed("core::result::Result::Ok")?;
        fn_body.group(Delimiter::Parenthesis, |ok_group| {
            ok_group.ident_str("Self");
            ok_group.group(Delimiter::Brace, |struct_body| {
                if let Some(fields) = self.fields.as_ref() {
                    for field in fields.names() {
                        let attributes = field
                            .attributes()
                            .get_attribute::<FieldAttributes>()?
                            .unwrap_or_default();
                        if attributes.skip {
                            if let Some((default_fn, _)) = attributes.default_fn {
                                struct_body.push_parsed(format!("{}: ({})(),", field, default_fn))?;
                            } else {
                                struct_body.push_parsed(format!(
                                    "{}: core::default::Default::default(),",
                                    field,
                                ))?;
                            }
                        } else if first_byte {
                            // `niche_field` ensures this field uses its own `BorrowDecode` impl.
                            first_byte = false;
                            struct_body.push_parsed(format!(
                                "{1}: {0}::BorrowDecode::<'_, {2}>::borrow_decode_with_first_byte(__first, decoder)?,",
                                crate_name, field, decode_context,
                            ))?;
                        } else if let Some(with) = attributes.with {
                            struct_body.push_parsed(format!(
                                "{}: {}::borrow_decode(decoder)?,",
                                field, with,
                            ))?;
                        } else if attributes.with_serde {
                            struct_body.push_parsed(format!(
                                "{1}: (<{0}::serde::BorrowCompat<_> as {0}::BorrowDecode::<'_, {2}>>::borrow_decode(decoder)?).0,",
                                crate_name, field, decode_context,
                            ))?;
                        } else {
                            struct_body.push_parsed(format!(
                                "{1}: {0}::BorrowDecode::<'_, {2}>::borrow_decode(decoder)?,",
                                crate_name, field, decode_context,
                            ))?;
                        }
                    }
                }
                Ok(())
            })?;
            Ok(())
        })?;
        Ok(())
    }
}

/// Returns `true` if `tokens` contain a lifetime, like `'a` or `'static`.
fn has_lifetime(tokens: &[TokenTree]) -> bool {
    tokens.iter().any(|token| match token {
        TokenTree::Punct(punct) => punct.as_char() == '\'',
        TokenTree::Group(group) => {
            let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
            has_lifetime(&tokens)
        }
        _ => false,
    })
}
