use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{Expr, ExprLit, ItemFn, Lit, LitStr, Token};

use iced_css_core::resolve::{Length, MarginValue, Resolved};
use iced_css_core::Stylesheet;

enum Source {
    Path(LitStr),
    Inline(LitStr),
}

struct Args {
    source: Source,
    policy: syn::Ident,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let source = if input.peek(LitStr) {
            Source::Path(input.parse()?)
        } else {
            let key: syn::Ident = input.parse()?;
            if key != "inline" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected path or `inline = \"...\"`",
                ));
            }
            let _: Token![=] = input.parse()?;
            Source::Inline(input.parse()?)
        };

        let _: Token![,] = input.parse()?;
        let key: syn::Ident = input.parse()?;
        if key != "policy" {
            return Err(syn::Error::new(key.span(), "expected `policy = ...`"));
        }
        let _: Token![=] = input.parse()?;
        let policy = input.parse()?;

        if !input.is_empty() {
            return Err(input.error("unexpected extra arguments"));
        }

        Ok(Args { source, policy })
    }
}

#[proc_macro_attribute]
pub fn style(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(args as Args);
    let mut function = syn::parse_macro_input!(item as ItemFn);

    match expand(&args, &mut function) {
        Ok(()) => quote!(#function).into(),
        Err(error) => {
            // Emit the item too, so unrelated diagnostics stay sane.
            let error = error.into_compile_error();
            quote!(#error #function).into()
        }
    }
}

fn expand(args: &Args, function: &mut ItemFn) -> syn::Result<()> {
    match args.policy.to_string().as_str() {
        "Compile" => {}
        "OnDemand" | "Auto" => {
            return Err(syn::Error::new(
                args.policy.span(),
                "not implemented yet",
            ));
        }
        other => {
            return Err(syn::Error::new(
                args.policy.span(),
                format!("unknown policy `{other}`; expected Compile, OnDemand, or Auto"),
            ));
        }
    }

    let (css, css_span) = match &args.source {
        Source::Inline(lit) => (lit.value(), lit.span()),
        Source::Path(lit) => {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let path = std::path::Path::new(&manifest_dir).join(lit.value());
            match std::fs::read_to_string(&path) {
                Ok(css) => (css, lit.span()),
                Err(error) => {
                    return Err(syn::Error::new(
                        lit.span(),
                        format!("cannot read stylesheet `{}`: {error}", path.display()),
                    ));
                }
            }
        }
    };

    let stylesheet = iced_css_core::parse(&css)
        .map_err(|error| syn::Error::new(css_span, error.to_string()))?;

    let mut rewriter = Rewriter {
        stylesheet,
        errors: Vec::new(),
    };
    rewriter.visit_item_fn_mut(function);

    match rewriter.errors.into_iter().reduce(|mut all, next| {
        all.combine(next);
        all
    }) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

struct Rewriter {
    stylesheet: Stylesheet,
    errors: Vec<syn::Error>,
}

impl VisitMut for Rewriter {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        syn::visit_mut::visit_expr_mut(self, expr);

        if let Expr::Macro(mac) = expr {
            use syn::punctuated::Punctuated;
            if let Ok(mut elements) = mac
                .mac
                .parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated)
            {
                for element in elements.iter_mut() {
                    self.visit_expr_mut(element);
                }
                mac.mac.tokens = quote!(#elements);
            }
            return;
        }

        let Expr::MethodCall(call) = expr else {
            return;
        };
        if call.method != "style_classes" || call.args.len() != 1 {
            return;
        }

        let classes = match literal_classes(&call.args[0]) {
            Ok(classes) => classes,
            Err(error) => {
                self.errors.push(error);
                return;
            }
        };

        for (class, span) in &classes {
            if !self.stylesheet.has_class(class) {
                self.errors.push(syn::Error::new(
                    *span,
                    format!("unknown class `{class}`: undefined in css"),
                ));
            }
        }
        if !self.errors.is_empty() {
            return;
        }

        let names: Vec<&str> = classes.iter().map(|(name, _)| name.as_str()).collect();
        let resolved = resolved_tokens(&self.stylesheet.resolve(&names));
        let receiver = &call.receiver;

        *expr = syn::parse_quote!(::iced_css::apply(#receiver, #resolved));
    }
}

fn literal_classes(arg: &Expr) -> syn::Result<Vec<(String, Span)>> {
    let Expr::Array(array) = arg else {
        return Err(syn::Error::new(
            arg.span(),
            "under the Compile policy, style_classes takes an array of string literals",
        ));
    };

    array
        .elems
        .iter()
        .map(|element| match element {
            Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) => Ok((lit.value(), lit.span())),
            other => Err(syn::Error::new(
                other.span(),
                "under the Compile policy, class names must be string literals",
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn expand_str(args: proc_macro2::TokenStream, function: proc_macro2::TokenStream) -> Result<String, String> {
        let args: Args = syn::parse2(args).map_err(|e| e.to_string())?;
        let mut function: ItemFn = syn::parse2(function).expect("test fn must parse");
        expand(&args, &mut function)
            .map(|()| function.to_token_stream().to_string())
            .map_err(|e| e.to_string())
    }

    fn view() -> proc_macro2::TokenStream {
        quote! {
            fn view() -> Element {
                container(text("t")).style_classes(["btn"])
            }
        }
    }

    #[test]
    fn valid_usage_rewrites_to_apply() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; height: 50px; }", policy = Compile),
            view(),
        )
        .unwrap();
        assert!(out.contains("iced_css :: apply"), "{out}");
        assert!(out.contains("Px (50f32)"), "{out}");
        assert!(!out.contains("style_classes"), "{out}");
    }

    #[test]
    fn rewrites_inside_column_macro() {
        let out = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn view() -> Element {
                    iced::widget::column![
                        container(text("a")).style_classes(["btn"]),
                        container(text("b")),
                    ].into()
                }
            },
        )
        .unwrap();
        assert!(out.contains("iced_css :: apply"), "{out}");
        assert!(!out.contains("style_classes"), "{out}");
    }

    #[test]
    fn invalid_css_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; height:", policy = Compile),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("CSS parse error"), "{err}");
    }

    #[test]
    fn unknown_class_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".other { width: 50px; }", policy = Compile),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("unknown class `btn`"), "{err}");
    }

    #[test]
    fn missing_file_is_a_build_error() {
        let err = expand_str(
            quote!("does/not/exist.css", policy = Compile),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("cannot read stylesheet"), "{err}");
    }

    #[test]
    fn dynamic_classes_are_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn view(class: &'static str) -> Element {
                    container(text("t")).style_classes([class])
                }
            },
        )
        .unwrap_err();
        assert!(err.contains("string literals"), "{err}");
    }

    #[test]
    fn non_array_class_list_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Compile),
            quote! {
                fn view(classes: [&'static str; 1]) -> Element {
                    container(text("t")).style_classes(classes)
                }
            },
        )
        .unwrap_err();
        assert!(err.contains("array of string literals"), "{err}");
    }

    #[test]
    fn runtime_policies_are_not_implemented_yet() {
        for policy in [quote!(OnDemand), quote!(Auto)] {
            let err = expand_str(
                quote!(inline = ".btn { width: 50px; }", policy = #policy),
                view(),
            )
            .unwrap_err();
            assert!(err.contains("not implemented yet"), "{err}");
        }
    }

    #[test]
    fn unknown_policy_is_a_build_error() {
        let err = expand_str(
            quote!(inline = ".btn { width: 50px; }", policy = Elsex),
            view(),
        )
        .unwrap_err();
        assert!(err.contains("unknown policy `Elsex`"), "{err}");
    }

    #[test]
    fn malformed_arguments_are_rejected() {
        assert!(expand_str(quote!(inline = ".btn {}"), view()).is_err());
        assert!(expand_str(quote!(inlined = ".btn {}", policy = Compile), view()).is_err());
        assert!(
            expand_str(quote!(inline = ".btn {}", policy = Compile, extra = 1), view()).is_err()
        );
    }
}

fn resolved_tokens(resolved: &Resolved) -> proc_macro2::TokenStream {
    let length = |value: &Option<Length>| match value {
        None => quote!(::core::option::Option::None),
        Some(Length::Px(v)) => quote!(::core::option::Option::Some(::iced_css::Length::Px(#v))),
        Some(Length::Percent(v)) => {
            quote!(::core::option::Option::Some(::iced_css::Length::Percent(#v)))
        }
        Some(Length::Auto) => quote!(::core::option::Option::Some(::iced_css::Length::Auto)),
    };
    let margin_value = |value: &MarginValue| match value {
        MarginValue::Px(v) => quote!(::iced_css::MarginValue::Px(#v)),
        MarginValue::Auto => quote!(::iced_css::MarginValue::Auto),
    };

    let width = length(&resolved.width);
    let height = length(&resolved.height);
    let min_width = length(&resolved.min_width);
    let max_width = length(&resolved.max_width);
    let margin = match &resolved.margin {
        None => quote!(::core::option::Option::None),
        Some(margins) => {
            let top = margin_value(&margins.top);
            let right = margin_value(&margins.right);
            let bottom = margin_value(&margins.bottom);
            let left = margin_value(&margins.left);
            quote!(::core::option::Option::Some(::iced_css::Margins {
                top: #top,
                right: #right,
                bottom: #bottom,
                left: #left,
            }))
        }
    };

    quote!(::iced_css::Resolved {
        width: #width,
        height: #height,
        min_width: #min_width,
        max_width: #max_width,
        margin: #margin,
    })
}
