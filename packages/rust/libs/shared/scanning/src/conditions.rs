use bloomery_model::SourceLocation;
use syn::{Attribute, Meta, punctuated::Punctuated};

/// Owned local syntax; interpretation belongs to the report's explicit context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfgExpression {
    Flag(String),
    Value(String, String),
    List(String, Vec<Self>),
    Attributes(Vec<Self>),
    All(Vec<Self>),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustCondition {
    pub test: SourceLocation,
    pub references: Vec<SourceLocation>,
    pub expression: CfgExpression,
}

fn syntax(meta: &Meta, depth: usize) -> CfgExpression {
    use CfgExpression::*;
    if depth > 64 {
        return Unsupported("cfg_recursion_limit".into());
    }
    let name = meta
        .path()
        .segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");
    match meta {
        Meta::Path(_) => Flag(name),
        Meta::NameValue(value) => match &value.value {
            syn::Expr::Lit(literal) => match &literal.lit {
                syn::Lit::Str(value) => Value(name, value.value()),
                _ => Unsupported(format!("invalid cfg value: {name}")),
            },
            _ => Unsupported(format!("invalid cfg value: {name}")),
        },
        Meta::List(list) => List(
            name,
            match list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated) {
                Ok(items) => items.iter().map(|item| syntax(item, depth + 1)).collect(),
                Err(_) => vec![Unsupported("malformed cfg syntax".into())],
            },
        ),
    }
}

pub(crate) fn attributes(attrs: &[Attribute]) -> CfgExpression {
    CfgExpression::Attributes(
        attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
            .map(|attr| syntax(&attr.meta, 0))
            .collect(),
    )
}

pub(crate) fn conditional_test(expression: &CfgExpression) -> bool {
    use CfgExpression::*;
    match expression {
        Attributes(items) => items.iter().any(conditional_test),
        List(name, items) if name == "cfg_attr" => items.iter().skip(1).any(conditional_test),
        Flag(name) | List(name, _) => name.split("::").last() == Some("test"),
        _ => false,
    }
}
