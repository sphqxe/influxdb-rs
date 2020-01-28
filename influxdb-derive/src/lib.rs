#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, Ident, NestedMeta, Data, Fields, DeriveInput};
use itertools::Itertools;
use quote::quote;
use quote::ToTokens;

// TODO: documentation

#[proc_macro_derive(Measurement, attributes(influx))]
pub fn derive_measurement(input: TokenStream) -> TokenStream {
    // Parse the string representation
    let ast = parse_macro_input!(input as DeriveInput);

    // Build the impl
    let gen = impl_measurement(&ast);

    // Return the generated impl
    TokenStream::from(gen.to_token_stream())
}

fn impl_measurement(input: &syn::DeriveInput) -> impl quote::ToTokens {

    let struct_fields = parse_influx_struct_fields(input);

    let name = &input.ident;

    let struct_attr = parse_influx_db_attrs(&input.attrs);
    let measurement_name = struct_attr.and_then(|a| a.name).unwrap_or_else(|| input.ident.to_string());

    let tags: Vec<_> = struct_fields.iter().filter(|f| f.is_tag()).collect();
    let fields: Vec<_> = struct_fields.iter().filter(|f| f.is_field()).collect();
    let timestamps: Vec<_> = struct_fields.iter().filter(|f| f.is_timestamp()).collect();

    if fields.is_empty() {
        panic!("InfluxDB requires that a measurement has at least one field");
    }

    if timestamps.len() > 1 {
        panic!("InfluxDB requires a maximum of one timestamp per measurement");
    }

    let name_and_tag_separator = if tags.is_empty() {
        quote!{ }
    } else {
        quote!{ v.push_str(","); }
    };

    let tag_stmts = tags.iter()
        .map(|field| {
            let field_name = field.field_name();
            let name = field.name();
            quote!{
                influxdb::measurement::Tag::new(#name, &self.#field_name.to_string()).append(v);
            }
        })
        .intersperse(quote!{ v.push_str(","); });

    let field_stmts = fields.iter()
        .map(|field| {
            let field_name = field.field_name();
            let name = field.name();
            quote!{
                influxdb::measurement::Field::new(#name, &self.#field_name).append(v);
            }
        })
        .intersperse(quote!{ v.push_str(","); });

    let timestamp_stmts = timestamps.iter()
        .map(|field| {
            let field_name = field.field_name();
            quote!{
                influxdb::measurement::Timestamp::new(self.#field_name).append(v);
            }
        });

    quote!{
        impl influxdb::Measurement for #name {
            fn to_data(&self, v: &mut String) {
                use std::string::ToString;

                v.push_str(#measurement_name);
                #name_and_tag_separator;
                #(#tag_stmts)*

                v.push_str(" ");

                #(#field_stmts)*

                v.push_str(" ");

                #(#timestamp_stmts)*
            }
        }
    }
}

fn parse_influx_struct_fields(input: &syn::DeriveInput) -> Vec<InfluxStructField> {
    match input.data {
        Data::Struct(ref ds) => {
            match ds.fields {
                Fields::Named(ref fields) => {
                    fields.named.iter().filter_map(|field| {
                        parse_influx_db_attrs(&field.attrs).map(|attr| {
                            let field_name = field.ident.clone().expect("All fields must be named");
                            InfluxStructField::new(field_name, attr)
                        })
                    }).collect()
                }
                _ => panic!("Fields must be named")
            }
        },
        _ => panic!("derive(Measurement) is only valid for structs")
    }
}

#[derive(Debug)]
struct InfluxStructField {
    field_name: Ident,
    attr: InfluxAttr,
}

impl InfluxStructField {
    fn new(field_name: Ident, attr: InfluxAttr) -> Self {
        InfluxStructField {
            field_name: field_name,
            attr: attr,
        }
    }

    fn field_name(&self) -> Ident {
        self.field_name.clone()
    }

    fn name(&self) -> String {
        self.attr.name.clone().unwrap_or_else(|| self.field_name.to_string())
    }

    fn is_tag(&self) -> bool { self.attr.is_tag }
    fn is_field(&self) -> bool { self.attr.is_field }
    fn is_timestamp(&self) -> bool { self.attr.is_timestamp }
}

#[derive(Debug, Default)]
struct InfluxAttr {
    name: Option<String>,
    is_tag: bool,
    is_field: bool,
    is_timestamp: bool,
}

impl InfluxAttr {
    fn merge(self, other: Self) -> Self {
        InfluxAttr {
            name: other.name.or(self.name),
            is_tag: other.is_tag || self.is_tag,
            is_field: other.is_field || self.is_field,
            is_timestamp: other.is_timestamp || self.is_timestamp,
        }
    }
}

fn parse_influx_db_attrs(attrs: &[syn::Attribute]) -> Option<InfluxAttr> {
    use syn::Meta;

    attrs.iter().filter_map(|attr| {
        if let Meta::List(ref metalist) = attr.parse_meta().unwrap() {
            let ident = metalist.path.get_ident().unwrap();
            let items = metalist.nested.iter();
            // #[influx(...)]
            if ident == "influx" {
                Some(parse_influx_db_attr(items))
            } else {
                None
            }
        } else { None }
    }).fold(None, |acc, attr| {
        match acc {
            Some(old_attr) => Some(old_attr.merge(attr)),
            None => Some(attr),
        }
    })
}

fn parse_influx_db_attr<'a>(items: impl IntoIterator<Item = &'a NestedMeta>) -> InfluxAttr {
    use syn::{Meta, NestedMeta, Lit::{Str}, StrStyle};

    let mut influx_attr = InfluxAttr::default();

    for item in items {
        match *item {
            NestedMeta::Meta(Meta::Path(ref path)) => {
                let ident = path.get_ident().unwrap();
                // #[influx(tag)]
                if ident == "tag" { influx_attr.is_tag = true }
                // #[influx(field)]
                if ident == "field" { influx_attr.is_field = true }
                // #[influx(timestamp)]
                if ident == "timestamp" { influx_attr.is_timestamp = true }
            }
            NestedMeta::Meta(Meta::NameValue(ref metanamevalue)) => {
                // #[influx(rename = "new_name")]
                let name = metanamevalue.path.get_ident().unwrap();
                if let Str(value) = &metanamevalue.lit {
                    if name == "rename" {
                        influx_attr.name = Some(value.value());
                    }
                } else {
                    panic!("Unknown 'influx' attribute found")
                }
            }
            _ => panic!("Unknown `influx` attribute found"),
        }
    }

    influx_attr
}
