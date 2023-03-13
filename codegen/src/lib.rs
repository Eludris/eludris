mod models;
use std::{env, fs, ops::Not};

use models::{EnumVariant, FieldInfo, ItemInfo, StructInfo};
use proc_macro::{Span, TokenStream};
use syn::{
    spanned::Spanned, Attribute, Error, Fields, Item, Lit, Meta, MetaNameValue, NestedMeta, Type,
    Variant,
};

use crate::models::EnumInfo;

macro_rules! unwrap {
    ($err:expr) => {
        match $err {
            Ok(res) => res,
            Err(err) => return err.to_compile_error().into(),
        }
    };
}

#[proc_macro_attribute]
pub fn autodoc(attr: TokenStream, item: TokenStream) -> TokenStream {
    if env::var("ELUDRIS_AUTODOC").is_ok() {
        let item = unwrap!(syn::parse::<Item>(item.clone()));
        let manifest_path = unwrap!(env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| Error::new(item.span(), "Could not find package manifest directory")));
        let package = unwrap!(env::var("CARGO_PKG_NAME")
            .map_err(|_| Error::new(item.span(), "Could not find package name")));
        let (info, name) = match item {
            Item::Fn(item) => {
                println!("fn {}", item.sig.ident);
                todo!()
            }
            Item::Enum(item) => {
                if !attr.is_empty() {
                    return Error::new(
                        unwrap!(syn::parse::<NestedMeta>(attr)).span(),
                        "Struct items expect no attribute args",
                    )
                    .to_compile_error()
                    .into();
                }
                let name = item.ident.to_string();
                let doc = unwrap!(get_doc(&item.attrs));
                let mut rename_all = None;
                let mut tag = None;
                let mut untagged = false;
                let mut content = None;
                for attr in item.attrs.iter().filter(|a| a.path.is_ident("serde")) {
                    if let Ok(Meta::List(meta)) = attr.parse_meta() {
                        for meta in meta.nested {
                            match meta {
                                NestedMeta::Meta(Meta::NameValue(meta)) => {
                                    if let Some(ident) = meta.path.get_ident() {
                                        match ident.to_string().as_str() {
                                            "rename_all" => {
                                                if let Lit::Str(lit) = meta.lit {
                                                    rename_all = Some(lit.value());
                                                }
                                            }
                                            "tag" => {
                                                if let Lit::Str(lit) = meta.lit {
                                                    tag = Some(lit.value());
                                                }
                                            }
                                            "content" => {
                                                if let Lit::Str(lit) = meta.lit {
                                                    content = Some(lit.value());
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                NestedMeta::Meta(Meta::Path(path)) => {
                                    if path.is_ident("untagged") {
                                        untagged = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                let mut variants = vec![];
                for variant in item.variants {
                    variants.push(unwrap!(get_variant(variant)));
                }
                (
                    ItemInfo::Enum(EnumInfo {
                        name: name.clone(),
                        doc,
                        content,
                        tag,
                        untagged,
                        rename_all,
                        variants,
                    }),
                    name,
                )
            }
            Item::Struct(item) => {
                let name = item.ident.to_string();
                let doc = unwrap!(get_doc(&item.attrs));
                let mut fields = vec![];
                for field in item.fields {
                    if let Type::Path(ty) = &field.ty {
                        let name = unwrap!(field.ident.as_ref().ok_or_else(|| {
                            Error::new(
                                field.span(),
                                "Cannot generate documentation for tuple struct fields",
                            )
                        }))
                        .to_string();
                        let field_type = unwrap!(ty.path.segments.last().ok_or_else(|| {
                            Error::new(ty.path.span(), "Cannot extract type from field")
                        }))
                        .ident
                        .to_string();
                        let doc = unwrap!(get_doc(&field.attrs));
                        let mut flattened = false;
                        for attr in field.attrs.iter().filter(|a| a.path.is_ident("serde")) {
                            if let Ok(Meta::List(meta)) = attr.parse_meta() {
                                for meta in meta.nested {
                                    if let NestedMeta::Meta(Meta::Path(path)) = meta {
                                        if path.is_ident("flatten") {
                                            flattened = true;
                                        } else if path.is_ident("skip") {
                                            continue;
                                        }
                                    }
                                }
                            }
                        }
                        fields.push(FieldInfo {
                            name,
                            field_type,
                            doc,
                            flattened,
                        })
                    } else {
                        return Error::new(
                            field.span(),
                            "Cannot document non-path typed struct fields",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
                let info = ItemInfo::Struct(StructInfo {
                    name: name.clone(),
                    doc,
                    fields,
                });
                (info, name)
            }
            item => {
                return Error::new(item.span(), "Unsupported item for autodoc")
                    .to_compile_error()
                    .into()
            }
        };
        unwrap!(fs::write(
            format!("{}/../autodoc/{}/{}.json", manifest_path, package, name),
            unwrap!(serde_json::to_string_pretty(&info).map_err(|_| Error::new(
                Span::call_site().into(),
                "Could not convert info into json"
            ))),
        )
        .map_err(|err| Error::new(
            Span::call_site().into(),
            format!("Could not write item info to filesystem: {}", err)
        )));
    };
    item
}

fn get_doc(attrs: &[Attribute]) -> Result<Option<String>, syn::Error> {
    let mut doc = String::new();

    for a in attrs.iter().filter(|a| a.path.is_ident("doc")) {
        let attr: MetaNameValue = match a.parse_meta()? {
            Meta::NameValue(attr) => attr,
            _ => unreachable!(),
        };
        if let Lit::Str(comment) = attr.lit {
            if !doc.is_empty() {
                doc.push('\n');
            };
            let comment = comment.value();
            if let Some(comment) = comment.strip_prefix(' ') {
                doc.push_str(comment);
            } else {
                doc.push_str(&comment);
            };
        }
    }

    Ok(doc.is_empty().not().then_some(doc))
}

fn get_variant(variant: Variant) -> Result<EnumVariant, syn::Error> {
    let doc = get_doc(&variant.attrs)?;
    let name = variant.ident.to_string();
    Ok(match variant.fields {
        Fields::Unit => EnumVariant::Unit { name, doc },
        Fields::Unnamed(fields) => {
            if fields.unnamed.len() > 1 {
                return Err(Error::new(
                    fields.span(),
                    "Cannot document tuple enum variants with more than one field",
                ));
            }
            let field = fields.unnamed.first().ok_or_else(|| {
                Error::new(
                    fields.span(),
                    "Tuple enum variants must have at least one field",
                )
            })?;
            if let Type::Path(ty) = &field.ty {
                let field_type = ty
                    .path
                    .segments
                    .last()
                    .ok_or_else(|| Error::new(ty.path.span(), "Cannot extract type from field"))?
                    .ident
                    .to_string();
                EnumVariant::Tuple {
                    name,
                    doc,
                    field_type,
                }
            } else {
                return Err(Error::new(
                    field.span(),
                    "Cannot document non-path typed struct fields",
                ));
            }
        }
        Fields::Named(struct_fields) => {
            let mut fields = vec![];
            for field in struct_fields.named {
                if let Type::Path(ty) = &field.ty {
                    let name = field
                        .ident
                        .as_ref()
                        .ok_or_else(|| {
                            Error::new(
                                field.span(),
                                "Cannot generate documentation for tuple struct fields",
                            )
                        })?
                        .to_string();
                    let field_type = ty
                        .path
                        .segments
                        .last()
                        .ok_or_else(|| {
                            Error::new(ty.path.span(), "Cannot extract type from field")
                        })?
                        .ident
                        .to_string();
                    let doc = get_doc(&field.attrs)?;
                    let mut flattened = false;
                    for attr in field.attrs.iter().filter(|a| a.path.is_ident("serde")) {
                        if let Ok(Meta::List(meta)) = attr.parse_meta() {
                            for meta in meta.nested {
                                if let NestedMeta::Meta(Meta::Path(path)) = meta {
                                    if path.is_ident("flatten") {
                                        flattened = true;
                                    } else if path.is_ident("skip") {
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    fields.push(FieldInfo {
                        name,
                        field_type,
                        doc,
                        flattened,
                    })
                } else {
                    return Err(Error::new(
                        field.span(),
                        "Cannot document non-path typed struct fields",
                    ));
                }
            }
            EnumVariant::Struct(StructInfo { name, doc, fields })
        }
    })
}
